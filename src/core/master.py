"""Master file indexing and polling for Excel-based snippet collections.

Design decisions:
- Uses file modification time polling instead of filesystem events for simplicity
  and cross-platform reliability
- Read-only workbook loading for performance and safety
- Graceful error handling with automatic retry on transient failures
"""

import os
import time
from datetime import datetime
from typing import List, Optional
from PySide6.QtCore import QThread, Signal


class MasterPollingThread(QThread):
    """Polls master files for changes using file modification time.
    
    Performance considerations:
    - Only stats files, doesn't read content
    - 1 second poll interval is sufficient for user-edited files
    - Uses dict for O(1) mtime lookups
    """
    
    file_changed = Signal(str)
    
    POLL_INTERVAL = 1.0  # seconds

    def __init__(self, watch_paths: List[str]):
        super().__init__()
        self._watch_paths = list(watch_paths)  # Copy to avoid mutation issues
        self._running = False
        self._last_mtimes: dict[str, float] = {}

    def update_paths(self, new_paths: List[str]):
        """Update the list of watched file paths.
        
        Thread-safe: Can be called from main thread while polling is running.
        """
        new_set = set(new_paths)
        self._watch_paths = list(new_paths)
        # Clean up mtimes for removed paths
        self._last_mtimes = {p: t for p, t in self._last_mtimes.items() if p in new_set}

    def run(self):
        """Main polling loop."""
        self._running = True
        
        while self._running:
            # Copy path list to avoid mutation during iteration
            paths = list(self._watch_paths)
            
            for path in paths:
                if not self._running:
                    break
                    
                if not os.path.exists(path):
                    continue
                    
                try:
                    mtime = os.path.getmtime(path)
                    
                    if path not in self._last_mtimes:
                        # First time seeing this file
                        self._last_mtimes[path] = mtime
                    elif mtime > self._last_mtimes[path]:
                        # File was modified
                        self._last_mtimes[path] = mtime
                        self.file_changed.emit(path)
                        
                except OSError:
                    # File may have been deleted/moved - ignore
                    pass
            
            # Interruptible sleep
            sleep_remaining = self.POLL_INTERVAL
            while sleep_remaining > 0 and self._running:
                time.sleep(min(0.2, sleep_remaining))
                sleep_remaining -= 0.2

    def stop(self):
        """Stop the polling thread gracefully."""
        self._running = False
        if not self.wait(2000):
            print("[WARN] Master polling thread did not stop gracefully")
            self.terminate()
            self.wait(1000)


class MasterManager:
    """Manages master file indexing and polling.
    
    Architecture:
    - Scans data/Master directory on startup for Excel files
    - Maintains indexed content in SQLite for fast searching
    - Polls for file changes and rebuilds index when modified
    """

    DEFAULT_MASTER_DIR = "data/Master"

    def __init__(self, db, master_dir: Optional[str] = None):
        """Initialize master manager.
        
        Args:
            db: Database instance for storing indexed content
            master_dir: Directory containing master Excel files
        """
        self.db = db
        self.master_dir = master_dir or self.DEFAULT_MASTER_DIR
        self._polling_thread: Optional[MasterPollingThread] = None
        
        # Ensure master directory exists
        self._ensure_directory()
        
        # Scan and register any new files
        self._scan_master_directory()
        
        # Start polling and initial index
        self._setup_polling()
        self.index_all()

    def _ensure_directory(self):
        """Create master directory if it doesn't exist."""
        try:
            os.makedirs(self.master_dir, exist_ok=True)
        except OSError as e:
            print(f"[WARN] Could not create master directory: {e}")

    def _scan_master_directory(self):
        """Find Excel files in the Master directory and register them in DB."""
        if not os.path.isdir(self.master_dir):
            return
            
        try:
            for filename in os.listdir(self.master_dir):
                if filename.lower().endswith(".xlsx") and not filename.startswith("~$"):
                    full_path = os.path.abspath(os.path.join(self.master_dir, filename))
                    try:
                        self.db.add_master_file(full_path)
                    except Exception as e:
                        print(f"[WARN] Could not register master file {filename}: {e}")
        except OSError as e:
            print(f"[WARN] Could not scan master directory: {e}")

    def _get_enabled_paths(self) -> List[str]:
        """Get list of enabled master file paths from database."""
        try:
            cursor = self.db.conn.cursor()
            cursor.execute("SELECT file_path FROM master_files WHERE is_enabled = 1")
            return [row['file_path'] for row in cursor.fetchall()]
        except Exception as e:
            print(f"[WARN] Could not get master file paths: {e}")
            return []

    def _setup_polling(self):
        """Initialize the polling thread."""
        paths = self._get_enabled_paths()
        
        self._polling_thread = MasterPollingThread(paths)
        self._polling_thread.file_changed.connect(self.rebuild_index)
        self._polling_thread.start()

    def refresh_watcher(self):
        """Update the list of watched files after settings change."""
        paths = self._get_enabled_paths()
        if self._polling_thread:
            self._polling_thread.update_paths(paths)

    def index_all(self):
        """Force rebuild index for all enabled master files."""
        for path in self._get_enabled_paths():
            self.rebuild_index(path)

    def rebuild_index(self, file_path: str):
        """Read Excel file and update database index.
        
        Args:
            file_path: Absolute path to the Excel file
        """
        if not os.path.exists(file_path):
            return

        try:
            # Get file ID from DB
            cursor = self.db.conn.cursor()
            cursor.execute("SELECT id FROM master_files WHERE file_path = ?", (file_path,))
            row = cursor.fetchone()
            if not row:
                return
            file_id = row['id']

            # Import openpyxl here to defer loading until needed
            try:
                import openpyxl
            except ImportError:
                print("[ERROR] openpyxl not installed - cannot read Excel files")
                return

            # Load workbook in read-only mode for performance
            try:
                wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
            except Exception as e:
                self._mark_file_error(file_id, str(e))
                print(f"[WARN] Error loading workbook {os.path.basename(file_path)}: {e}")
                return

            ws = wb.active
            if ws is None:
                self._mark_file_error(file_id, "No active sheet found")
                print(f"[WARN] No active sheet in {os.path.basename(file_path)}")
                try:
                    wb.close()
                except Exception:
                    pass
                return
            
            # Extract items from first column
            items = []
            try:
                for idx, row in enumerate(ws.iter_rows(min_col=1, max_col=1, values_only=True), start=1):
                    content = row[0]
                    if content is not None:
                        text = str(content).strip()
                        if text:  # Only add non-empty items
                            items.append((text, idx))
            finally:
                try:
                    wb.close()
                except Exception:
                    pass
            
            # Update database
            self.db.update_master_items(file_id, items)
            
            # Clear any previous error
            cursor.execute(
                "UPDATE master_files SET last_error = NULL, is_enabled = 1 WHERE id = ?",
                (file_id,)
            )
            self.db.conn.commit()
            
            print(f"[*] Indexed {os.path.basename(file_path)}: {len(items)} items")
            
        except Exception as e:
            self._mark_file_error_by_path(file_path, str(e))
            print(f"[ERROR] Failed to index {file_path}: {e}")

    def _mark_file_error(self, file_id: int, error: str):
        """Mark a master file as having an error."""
        try:
            cursor = self.db.conn.cursor()
            cursor.execute(
                "UPDATE master_files SET last_error = ?, is_enabled = 0 WHERE id = ?",
                (error[:500], file_id)  # Truncate long errors
            )
            self.db.conn.commit()
        except Exception:
            pass

    def _mark_file_error_by_path(self, file_path: str, error: str):
        """Mark a master file as having an error by path."""
        try:
            cursor = self.db.conn.cursor()
            cursor.execute(
                "UPDATE master_files SET last_error = ?, is_enabled = 0 WHERE file_path = ?",
                (error[:500], file_path)
            )
            self.db.conn.commit()
        except Exception:
            pass

    def stop(self):
        """Stop the polling thread."""
        if self._polling_thread:
            self._polling_thread.stop()
            self._polling_thread = None
