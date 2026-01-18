"""Main application controller for Shift Paste."""

from PySide6.QtWidgets import QApplication, QSystemTrayIcon, QMenu, QMessageBox
from PySide6.QtGui import QIcon
from PySide6.QtCore import QTimer, Qt
import sys
import keyboard
import pyperclip
from pathlib import Path
from typing import Optional
import platform

from src.ui.main_window import MainWindow
from src.ui.settings_window import SettingsWindow
from src.core.clipboard_monitor import ClipboardMonitor
from src.core.search_engine import FuzzySearchEngine
from src.data.database import Database
from src.data.excel_manager import ExcelManager
from src.data.config_manager import ConfigManager


class ShiftPasteApp:
    """Main application controller."""

    def __init__(self):
        """Initialize application."""
        self.app = QApplication(sys.argv)
        self.app.setQuitOnLastWindowClosed(False)

        # Initialize components
        self.config = ConfigManager()
        self.db = Database()
        self.search_engine = FuzzySearchEngine()
        self.excel_manager = ExcelManager(
            self.config.get('master_file.directory', 'data/Master')
        )

        # UI
        self.main_window: Optional[MainWindow] = None
        self.settings_window: Optional[SettingsWindow] = None
        self.tray_icon: Optional[QSystemTrayIcon] = None

        # Services
        self.clipboard_monitor: Optional[ClipboardMonitor] = None
        self.excel_watcher_timer: Optional[QTimer] = None

        # State
        self.is_window_open = False
        self.last_hotkey = ""

        self._setup_ui()
        self._setup_services()
        self._setup_hotkeys()
        self._load_master_files()

    def _setup_ui(self):
        """Initialize UI."""
        preview_chars = self.config.get('clipboard.preview_chars', 100)
        self.main_window = MainWindow(preview_chars)
        # Use QueuedConnection to ensure UI updates happen in main thread
        self.main_window.search_changed.connect(self._on_search_changed, Qt.QueuedConnection)
        self.main_window.item_selected.connect(self._on_item_selected, Qt.QueuedConnection)
        self.main_window.item_deleted.connect(self._on_item_deleted, Qt.QueuedConnection)
        self.main_window.window_closed.connect(self._on_window_closed, Qt.QueuedConnection)

        self._setup_tray_icon()

    def _setup_tray_icon(self):
        """Create system tray icon."""
        # Try to find app icon
        icon_path = Path("resources/icons/app_icon.png")
        if not icon_path.exists():
            # Create a simple colored square icon as fallback
            from PySide6.QtGui import QPixmap, QColor
            pixmap = QPixmap(64, 64)
            pixmap.fill(QColor("#0078d4"))
            icon = QIcon(pixmap)
        else:
            icon = QIcon(str(icon_path))

        self.tray_icon = QSystemTrayIcon(icon)

        tray_menu = QMenu()

        open_action = tray_menu.addAction("Open Shift Paste")
        open_action.triggered.connect(self.show_main_window)

        settings_action = tray_menu.addAction("Settings")
        settings_action.triggered.connect(self.show_settings)

        tray_menu.addSeparator()

        clear_action = tray_menu.addAction("Clear Clipboard History")
        clear_action.triggered.connect(self._clear_clipboard)

        tray_menu.addSeparator()

        quit_action = tray_menu.addAction("Quit")
        quit_action.triggered.connect(self.quit_app)

        self.tray_icon.setContextMenu(tray_menu)
        self.tray_icon.activated.connect(self._on_tray_activated)
        self.tray_icon.show()

    def _setup_services(self):
        """Initialize background services."""
        # Clipboard monitor
        self.clipboard_monitor = ClipboardMonitor()
        # Use QueuedConnection to ensure clipboard changes are processed in main thread
        self.clipboard_monitor.clipboard_changed.connect(self._on_clipboard_changed, Qt.QueuedConnection)
        self.clipboard_monitor.start()

        # Excel file watcher
        if self.config.get('master_file.auto_reload', True):
            self.excel_manager.start_watching()
            file_changed_signal = self.excel_manager.get_file_changed_signal()
            if file_changed_signal:
                # Use QueuedConnection for thread safety
                file_changed_signal.connect(self._on_excel_changed, Qt.QueuedConnection)

    def _setup_hotkeys(self):
        """Register global hotkeys."""
        system = platform.system()

        if system == "Windows":
            hotkey = self.config.get('shortcuts.windows', 'ctrl+shift+v')
        elif system == "Darwin":
            hotkey = self.config.get('shortcuts.macos', 'shift+cmd+v')
        else:
            hotkey = self.config.get('shortcuts.linux', 'shift+super+v')

        try:
            # Wrap hotkey callback to ensure it runs safely in main thread
            def hotkey_callback():
                # Use QTimer to schedule the toggle in the main thread
                QTimer.singleShot(0, self.toggle_main_window)
            
            keyboard.add_hotkey(hotkey, hotkey_callback, suppress=True)
            self.last_hotkey = hotkey
            print(f"Hotkey registered: {hotkey}")
        except Exception as e:
            print(f"Error registering hotkey: {e}")
            QMessageBox.warning(
                None,
                "Hotkey Error",
                f"Failed to register hotkey '{hotkey}'.\n\n{str(e)}"
            )

    def _load_master_files(self):
        """Load all master Excel files."""
        categories = self.excel_manager.get_all_categories()

        for category in categories:
            items = self.excel_manager.import_from_excel(category)
            self.db.sync_master_from_excel(category, items)

    def _on_clipboard_changed(self, content: str):
        """Handle clipboard content change.

        Args:
            content: New clipboard content
        """
        max_items = self.config.get('clipboard.max_items', 20)
        item_id = self.db.add_clipboard_item(content)

        if item_id > 0:  # Not a duplicate
            # Trim old items if over limit
            recent = self.db.get_recent_items(limit=max_items + 100)
            if len(recent) > max_items:
                items_to_delete = recent[max_items:]
                for item in items_to_delete:
                    self.db.delete_clipboard_item(item['id'])

    def _on_excel_changed(self, filepath: str):
        """Handle external Excel file changes.

        Args:
            filepath: Path to changed Excel file
        """
        from pathlib import Path
        category = Path(filepath).stem

        items = self.excel_manager.import_from_excel(category)
        self.db.sync_master_from_excel(category, items)

        print(f"Reloaded {category}.xlsx")

    def show_main_window(self):
        """Show main window with recent items."""
        # Load recent items
        max_items = self.config.get('clipboard.max_items', 20)
        recent = self.db.get_recent_items(max_items)
        master = self.db.get_all_master_items()

        # Combine and rank by recency
        all_items = recent + master
        all_items.sort(key=lambda x: x.get('timestamp', 0), reverse=True)

        self.main_window.update_results(all_items)
        self.main_window.show_near_cursor()
        self.is_window_open = True

    def toggle_main_window(self):
        """Toggle window visibility."""
        if self.is_window_open and self.main_window.isVisible():
            self.main_window.close()
            self.is_window_open = False
        else:
            self.show_main_window()

    def show_settings(self):
        """Show settings window."""
        if not self.settings_window:
            self.settings_window = SettingsWindow(self.config, None)
            self.settings_window.settings_changed.connect(self._on_settings_changed)

        self.settings_window.show()
        self.settings_window.activateWindow()

    def _on_search_changed(self, query: str):
        """Handle search query changes.

        Args:
            query: Search query string
        """
        if query.strip():
            # Search mode
            results = self.db.search_all_items(query, max_results=50)

            # Create items list for fuzzy ranking
            items_for_fuzzy = []
            for result in results:
                items_for_fuzzy.append({
                    'content': result['content'],
                    'timestamp': result['timestamp'],
                    'source_table': result['source_table'],
                    'category': result.get('category'),
                    'notes': result.get('notes'),
                    'id': result['source_id'],
                    'source_app': result.get('source_app')
                })

            # Apply fuzzy ranking
            matches = self.search_engine.search(query, items_for_fuzzy, max_results=20)

            # Convert matches to dict format
            ranked_results = [match.metadata for match in matches]
            self.main_window.update_results(ranked_results)
        else:
            # No search - show recent items
            max_items = self.config.get('clipboard.max_items', 20)
            recent = self.db.get_recent_items(max_items)
            master = self.db.get_all_master_items()

            all_items = recent + master
            all_items.sort(key=lambda x: x.get('timestamp', 0), reverse=True)

            self.main_window.update_results(all_items[:20])

    def _on_item_selected(self, item: dict):
        """Handle item selection - paste and close.

        Args:
            item: Selected item dictionary
        """
        content = item.get('content', '')

        # Close window
        self.main_window.close()
        self.is_window_open = False

        # Copy to clipboard
        pyperclip.copy(content)

        # Simulate Ctrl+V
        try:
            import pyautogui
            import time
            time.sleep(0.05)  # Small delay
            pyautogui.hotkey('ctrl', 'v')
        except Exception as e:
            print(f"Error pasting: {e}")
            # Just leave content in clipboard
            QMessageBox.information(
                None,
                "Content Copied",
                f"Content copied to clipboard:\n\n{content[:100]}..."
            )

    def _on_item_deleted(self, item_id: int):
        """Handle item deletion.

        Args:
            item_id: ID of item to delete
        """
        self.db.delete_clipboard_item(item_id)

    def _on_window_closed(self):
        """Handle window close."""
        self.is_window_open = False

    def _on_tray_activated(self, reason):
        """Handle tray icon click.

        Args:
            reason: Activation reason
        """
        # Handle both single and double clicks
        if reason == QSystemTrayIcon.Trigger or reason == QSystemTrayIcon.DoubleClick:
            self.toggle_main_window()

    def _on_settings_changed(self):
        """Handle settings changes."""
        # Reload config
        self.config.load()

        # Re-setup hotkeys if they changed
        old_hotkey = self.last_hotkey
        system = platform.system()

        if system == "Windows":
            new_hotkey = self.config.get('shortcuts.windows', 'ctrl+shift+v')
        elif system == "Darwin":
            new_hotkey = self.config.get('shortcuts.macos', 'shift+cmd+v')
        else:
            new_hotkey = self.config.get('shortcuts.linux', 'shift+super+v')

        if new_hotkey != old_hotkey:
            try:
                keyboard.remove_hotkey(old_hotkey)
                
                # Wrap hotkey callback to ensure it runs safely
                def hotkey_callback():
                    from PySide6.QtCore import QTimer
                    QTimer.singleShot(0, self.toggle_main_window)
                
                keyboard.add_hotkey(new_hotkey, hotkey_callback, suppress=True)
                self.last_hotkey = new_hotkey
                print(f"Hotkey changed: {old_hotkey} -> {new_hotkey}")
            except Exception as e:
                print(f"Error updating hotkey: {e}")

    def _clear_clipboard(self):
        """Clear clipboard history."""
        reply = QMessageBox.question(
            None,
            'Clear History',
            'Clear all clipboard history?\n\n(Master files will not be affected)',
            QMessageBox.Yes | QMessageBox.No
        )

        if reply == QMessageBox.Yes:
            self.db.clear_clipboard_history()
            print("Clipboard history cleared")

    def quit_app(self):
        """Cleanup and quit."""
        if self.clipboard_monitor:
            self.clipboard_monitor.stop()
            self.clipboard_monitor.wait()

        self.excel_manager.stop_watching()

        if self.db.conn:
            self.db.close()

        self.app.quit()

    def run(self):
        """Start the application.

        Returns:
            Application exit code
        """
        return self.app.exec()
