"""Main application controller for Shift Paste."""

from PySide6.QtWidgets import QApplication, QSystemTrayIcon, QMenu, QMessageBox
from PySide6.QtGui import QIcon
from PySide6.QtCore import QTimer, Qt, QAbstractNativeEventFilter, QCoreApplication
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

        # Load and set application icon early so windows/taskbar pick it up
        self._load_app_icon()

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
        # Windows native hotkey state
        self._hotkey_registered = False
        self._hotkey_id = 1
        self._native_filter = None

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

    def _load_app_icon(self):
        """Load application icon and set it on QApplication."""
        from PySide6.QtGui import QIcon
        icon_path = Path("resources/icons/app_icon.png")
        try:
            if icon_path.exists():
                icon = QIcon(str(icon_path))
            else:
                from PySide6.QtGui import QPixmap, QColor
                pixmap = QPixmap(64, 64)
                pixmap.fill(QColor("#0078d4"))
                icon = QIcon(pixmap)

            try:
                self.app.setWindowIcon(icon)
            except Exception:
                pass
        except Exception:
            pass

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

        # Apply icon to application and windows so taskbar shows same icon
        try:
            self.app.setWindowIcon(icon)
        except Exception:
            pass

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

        # Ensure main window (if created) has same icon
        try:
            if self.main_window:
                self.main_window.setWindowIcon(icon)
        except Exception:
            pass

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
            if system == "Windows":
                # Use native RegisterHotKey to avoid external dependency issues
                import ctypes
                user32 = ctypes.windll.user32

                def _parse_hotkey(hk: str):
                    parts = hk.lower().split('+')
                    mods = 0
                    vk = 0
                    MODIFIERS = {
                        'alt': 0x0001,
                        'ctrl': 0x0002,
                        'control': 0x0002,
                        'shift': 0x0004,
                        'win': 0x0008,
                        'super': 0x0008,
                        'cmd': 0x0008
                    }

                    for p in parts:
                        p = p.strip()
                        if p in MODIFIERS:
                            mods |= MODIFIERS[p]
                        else:
                            # Take first character as virtual key for letters
                            if len(p) == 1:
                                vk = ord(p.upper())
                            else:
                                # Try common names
                                vk = ord(p[0].upper()) if p[0].isalpha() else 0

                    return mods, vk

                mods, vk = _parse_hotkey(hotkey)
                if vk == 0:
                    raise ValueError(f"Could not parse hotkey: {hotkey}")

                if not user32.RegisterHotKey(None, self._hotkey_id, mods, vk):
                    # Fallback to 'keyboard' library if native registration fails
                    try:
                        def hotkey_callback():
                            QTimer.singleShot(0, self.toggle_main_window)

                        keyboard.add_hotkey(hotkey, hotkey_callback, suppress=True)
                        self.last_hotkey = hotkey
                        print(f"Fallback hotkey registered (keyboard lib): {hotkey}")
                        return
                    except Exception:
                        raise OSError("RegisterHotKey failed")

                class _NativeFilter(QAbstractNativeEventFilter):
                    def __init__(self, outer):
                        super().__init__()
                        self.outer = outer

                    def nativeEventFilter(self, eventType, message):
                        try:
                            msg = ctypes.wintypes.MSG.from_address(int(message))
                            # WM_HOTKEY == 0x0312
                            if msg.message == 0x0312:
                                QTimer.singleShot(0, self.outer.toggle_main_window)
                                return True, 0
                        except Exception:
                            pass
                        return False, 0

                self._native_filter = _NativeFilter(self)
                QCoreApplication.instance().installNativeEventFilter(self._native_filter)
                self._hotkey_registered = True
                self.last_hotkey = hotkey
                print(f"Native hotkey registered: {hotkey}")
            else:
                # Fallback to keyboard module for non-Windows platforms
                def hotkey_callback():
                    QTimer.singleShot(0, self.toggle_main_window)

                keyboard.add_hotkey(hotkey, hotkey_callback, suppress=True)
                self.last_hotkey = hotkey
                print(f"Hotkey registered (keyboard lib): {hotkey}")
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
        # Tell clipboard monitor to ignore the change caused by our own copy
        try:
            if self.clipboard_monitor:
                self.clipboard_monitor.ignore_next_change()
        except Exception:
            pass

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
        old_master_dir = None
        try:
            old_master_dir = str(self.excel_manager.master_directory)
        except Exception:
            old_master_dir = None

        self.config.load()

        # Handle master file directory change
        new_master_dir = self.config.get('master_file.directory', 'data/Master')
        if old_master_dir != new_master_dir:
            try:
                # Stop previous watcher and replace manager
                if self.excel_manager:
                    self.excel_manager.stop_watching()

                self.excel_manager = ExcelManager(new_master_dir)

                # Start watching if enabled
                if self.config.get('master_file.auto_reload', True):
                    self.excel_manager.start_watching()

                # Reload master files into DB
                self._load_master_files()
                print(f"Master directory changed: {old_master_dir} -> {new_master_dir}")
            except Exception as e:
                print(f"Error updating master directory: {e}")

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
                    system = platform.system()
                    if system == "Windows" and self._hotkey_registered:
                        # Unregister native hotkey
                        import ctypes
                        user32 = ctypes.windll.user32
                        user32.UnregisterHotKey(None, self._hotkey_id)
                        if self._native_filter:
                            QCoreApplication.instance().removeNativeEventFilter(self._native_filter)
                            self._native_filter = None
                        self._hotkey_registered = False
                    else:
                        try:
                            keyboard.remove_hotkey(old_hotkey)
                        except Exception:
                            pass

                    # Register new hotkey by re-calling setup
                    self._setup_hotkeys()
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

        # Unregister native hotkey on Windows
        try:
            if platform.system() == "Windows" and self._hotkey_registered:
                import ctypes
                user32 = ctypes.windll.user32
                user32.UnregisterHotKey(None, self._hotkey_id)
                if self._native_filter:
                    QCoreApplication.instance().removeNativeEventFilter(self._native_filter)
                    self._native_filter = None
                self._hotkey_registered = False
        except Exception:
            pass

        self.app.quit()

    def run(self):
        """Start the application.

        Returns:
            Application exit code
        """
        return self.app.exec()
