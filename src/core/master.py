import os
import time
from datetime import datetime
from typing import List, Dict, Set
from PySide6.QtCore import QThread, Signal
import openpyxl

class MasterPollingThread(QThread):
    """Polls master files for changes using mtime."""
    file_changed = Signal(str)

    def __init__(self, watch_paths: List[str]):
        super().__init__()
        self.watch_paths = watch_paths
        self.running = False
        self.last_mtimes = {}

    def update_paths(self, new_paths: List[str]):
        self.watch_paths = new_paths
        # Remove old paths from mtimes to keep it clean
        current_set = set(new_paths)
        self.last_mtimes = {p: t for p, t in self.last_mtimes.items() if p in current_set}

    def run(self):
        self.running = True
        while self.running:
            for path in self.watch_paths:
                if os.path.exists(path):
                    try:
                        mtime = os.path.getmtime(path)
                        if path not in self.last_mtimes:
                            self.last_mtimes[path] = mtime
                        elif mtime > self.last_mtimes[path]:
                            self.last_mtimes[path] = mtime
                            self.file_changed.emit(path)
                    except OSError:
                        pass
            time.sleep(1.0)

    def stop(self):
        self.running = False
        self.wait()

class MasterManager:
    """Manages master file indexing and polling."""

    def __init__(self, db, master_dir: str = "data/Master"):
        self.db = db
        self.master_dir = master_dir
        self.polling_thread = None
        
        # 1. Ensure master directory exists
        if not os.path.exists(self.master_dir):
            os.makedirs(self.master_dir, exist_ok=True)
            
        # 2. Scan directory and register any new files found there
        self._scan_master_directory()
        
        # 3. Startup polling and initial index
        self._set_up_polling()
        self.index_all()

    def _scan_master_directory(self):
        """Find Excel files in the Master directory and ensure they are in the DB."""
        for filename in os.listdir(self.master_dir):
            if filename.endswith(".xlsx"):
                full_path = os.path.abspath(os.path.join(self.master_dir, filename))
                self.db.add_master_file(full_path)

    def _set_up_polling(self):
        """Initialize the polling thread."""
        cursor = self.db.conn.cursor()
        cursor.execute("SELECT file_path FROM master_files WHERE is_enabled = 1")
        paths = [row['file_path'] for row in cursor.fetchall()]
        
        self.polling_thread = MasterPollingThread(paths)
        self.polling_thread.file_changed.connect(self.rebuild_index)
        self.polling_thread.start()

    def refresh_watcher(self):
        """Update the list of watched files."""
        cursor = self.db.conn.cursor()
        cursor.execute("SELECT file_path FROM master_files WHERE is_enabled = 1")
        paths = [row['file_path'] for row in cursor.fetchall()]
        if self.polling_thread:
            self.polling_thread.update_paths(paths)

    def index_all(self):
        """Force rebuild index for all enabled master files."""
        cursor = self.db.conn.cursor()
        cursor.execute("SELECT file_path FROM master_files WHERE is_enabled = 1")
        for row in cursor.fetchall():
            self.rebuild_index(row['file_path'])

    def rebuild_index(self, file_path: str):
        """Read Excel file and update database index."""
        if not os.path.exists(file_path):
            return

        try:
            # Get file ID from DB
            cursor = self.db.conn.cursor()
            cursor.execute("SELECT id FROM master_files WHERE file_path = ?", (file_path,))
            row = cursor.fetchone()
            if not row: return
            file_id = row['id']

            # Read Excel
            # Using data_only=True to get values, not formulas
            try:
                wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
            except Exception as e:
                 print(f"[!] Error loading workbook {file_path}: {e}")
                 return

            ws = wb.active
            if ws is None:
                print(f"[!] No active sheet in {file_path}")
                return
            
            items = []
            for idx, row in enumerate(ws.iter_rows(min_col=1, max_col=1, values_only=True), start=1):
                content = row[0]
                if content is not None and str(content).strip():
                    items.append((str(content).strip(), idx))
            
            # Update DB
            self.db.update_master_items(file_id, items)
            print(f"[*] Rebuilt index for {os.path.basename(file_path)}: {len(items)} items.")
            
        except Exception as e:
            cursor = self.db.conn.cursor()
            cursor.execute("UPDATE master_files SET last_error = ?, is_enabled = 0 WHERE file_path = ?", (str(e), file_path))
            self.db.conn.commit()
            print(f"[!] Error rebuilding index for {file_path}: {e}")

    def stop(self):
        """Stop the polling thread."""
        if self.polling_thread:
            self.polling_thread.stop()
            self.polling_thread.wait()
