"""Master file monitoring and Excel processing."""

import os
from datetime import datetime
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler
import openpyxl
from typing import List, Tuple, Callable

class MasterFileHandler(FileSystemEventHandler):
    """Handles file system events for master Excel files."""

    def __init__(self, watch_paths: List[str], on_change_callback: Callable[[str], None]):
        """
        Args:
            watch_paths: List of absolute file paths to watch
            on_change_callback: Function to call when a watched file changes
        """
        self.watch_paths = [os.path.abspath(p) for p in watch_paths]
        self.on_change_callback = on_change_callback

    def on_modified(self, event):
        if event.is_directory:
            return
        
        event_path = os.path.abspath(event.src_path)
        if event_path in self.watch_paths:
            # Small delay to allow file lock release
            import time
            time.sleep(0.5)
            self.on_change_callback(event_path)


class MasterManager:
    """Manages master file indexing and watching."""

    def __init__(self, db):
        self.db = db
        self.observer = Observer()
        self.watch_paths = []
        self._set_up_watcher()

    def _set_up_watcher(self):
        """Initialize the observer and handler."""
        # Get enabled master files from DB
        cursor = self.db.conn.cursor()
        cursor.execute("SELECT file_path FROM master_files WHERE is_enabled = 1")
        self.watch_paths = [row['file_path'] for row in cursor.fetchall()]
        
        if not self.watch_paths:
            return

        # Watch directories containing the files
        dirs_to_watch = set(os.path.dirname(p) for p in self.watch_paths)
        handler = MasterFileHandler(self.watch_paths, self.rebuild_index)
        
        for d in dirs_to_watch:
            if os.path.exists(d):
                self.observer.schedule(handler, d, recursive=False)
        
        self.observer.start()

    def rebuild_index(self, file_path: str):
        """Read Excel file and update database index."""
        try:
            # Get file ID from DB
            cursor = self.db.conn.cursor()
            cursor.execute("SELECT id FROM master_files WHERE file_path = ?", (file_path,))
            row = cursor.fetchone()
            if not row:
                return
            file_id = row['id']

            # Read Column A only
            wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
            ws = wb.active
            
            items: List[Tuple[str, int]] = []
            for idx, row in enumerate(ws.iter_rows(min_col=1, max_col=1, values_only=True), start=1):
                content = row[0]
                if content:
                    items.append((str(content), idx))
            
            # Update DB
            self.db.update_master_items(file_id, items)
            print(f"Rebuilt index for {file_path}: {len(items)} items found.")
            
        except Exception as e:
            # Update DB with error
            cursor = self.db.conn.cursor()
            cursor.execute("UPDATE master_files SET last_error = ?, is_enabled = 0 WHERE file_path = ?", (str(e), file_path))
            self.db.conn.commit()
            print(f"Error rebuilding index for {file_path}: {e}")

    def stop(self):
        """Stop the file watcher."""
        if self.observer.is_alive():
            self.observer.stop()
            self.observer.join()
