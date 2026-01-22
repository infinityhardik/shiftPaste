"""Main application controller for Shift Paste."""

import sys
from typing import Optional

from PySide6.QtWidgets import QApplication, QMessageBox
from PySide6.QtCore import Qt, QCoreApplication, QObject

from src.ui.main_window import MainWindow
from src.ui.settings_window import SettingsWindow
from src.ui.tray import SystemTrayManager
from src.core.clipboard_monitor import ClipboardMonitor
from src.core.search_engine import FuzzySearchEngine
from src.core.master import MasterManager
from src.core.paste import PasteManager
from src.data.database import Database
from src.utils.hotkey import HotkeyManager
from src.utils.autostart import set_autostart


class ShiftPasteApp(QObject):
    """Main application controller."""

    def __init__(self):
        """Initialize application and its components."""
        super().__init__()
        try:
            self.app = QApplication(sys.argv)
            self.app.setQuitOnLastWindowClosed(False)

            print("[*] Initializing Database...")
            self.db = Database()
            
            print("[*] Starting Search Engine and Paste Manager...")
            self.search_engine = FuzzySearchEngine()
            self.paste_manager = PasteManager()
            self.master_manager = MasterManager(self.db)
            
            print("[*] Starting Clipboard Monitor...")
            self.clipboard_monitor = ClipboardMonitor()
            self.clipboard_monitor.clipboard_changed.connect(self._on_clipboard_changed)
            self.clipboard_monitor.start()

            print("[*] Initializing UI...")
            self.main_window = MainWindow()
            self.main_window.search_changed.connect(self._on_search_changed)
            self.main_window.item_selected.connect(self._on_item_selected)
            self.main_window.settings_requested.connect(self.show_settings)
            self.main_window.clear_requested.connect(self._clear_history)
            
            self.tray_manager = SystemTrayManager()
            self.tray_manager.show_requested.connect(self.show_main_window)
            self.tray_manager.settings_requested.connect(self.show_settings)
            self.tray_manager.clear_requested.connect(self._clear_history)
            self.tray_manager.exit_requested.connect(self.quit_app)

            # 5. Hotkey Management
            print("[*] Registering Hotkeys...")
            shortcut = self.db.get_setting('hotkey', 'Ctrl+Shift+V')
            excluded_apps = [a for a in self.db.get_setting('excluded_apps', '').split(',') if a.strip()]
            self.hotkey_manager = HotkeyManager(shortcut, self.toggle_main_window, excluded_apps)
            self.hotkey_manager.triggered.connect(self.toggle_main_window, Qt.ConnectionType.QueuedConnection)
            self.hotkey_manager.start()
            
            print("[*] Shift Paste is ready.")
        except Exception as e:
            print(f"[!] CRITICAL ERROR during startup: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)

    def show_main_window(self):
        """Display the main popup near the cursor."""
        # Capture the window that was active before showing our UI
        self.paste_manager.capture_active_window()
        
        # Load initial items (recent clipboard)
        items = self.db.get_recent_items(20)
        # Apply time ago strings
        for item in items:
            item['time_ago'] = self.search_engine.get_time_ago_string(item['last_copied_at'])
            
        self.main_window.update_results(items)
        self.main_window.show_near_cursor()

    def toggle_main_window(self):
        """Toggle the clipboard window."""
        if self.main_window.isVisible():
            self.main_window.close()
        else:
            self.show_main_window()

    def show_settings(self):
        """Open the settings modal."""
        self.settings_window = SettingsWindow(self.db)
        self.settings_window.settings_changed.connect(self._on_settings_updated)
        self.settings_window.exec()

    def _on_clipboard_changed(self, text, is_formatted, formatted_content):
        """Handle new clipboard content detected by monitor."""
        self.db.add_clipboard_item(text, is_formatted, formatted_content)

    def _on_search_changed(self, query):
        """Execute search and update UI."""
        if not query:
            items = self.db.get_recent_items(20)
        else:
            # Search both clipboard and masters
            clip_res = self.db.search_clipboard(query, limit=50)
            master_res = self.db.search_masters(query, limit=50)
            
            # Rank and merge
            all_res = clip_res + master_res
            # returns list of dicts directly
            items = self.search_engine.rank_search_results(all_res, query)
            # Take top 20
            items = items[:20]

        # Enrich with time ago
        for item in items:
            ts = item.get('last_copied_at') or item.get('master_modified')
            item['time_ago'] = self.search_engine.get_time_ago_string(ts)

        self.main_window.update_results(items)

    def _on_item_selected(self, item):
        """Process item selection and paste."""
        content = item.get('content', '')
        
        # Temporarily ignore clipboard change to avoid re-adding our own paste
        self.clipboard_monitor.ignore_next_change()
        
        import pyperclip
        pyperclip.copy(content)
        
        # Use paste manager to restore focus and simulate keypress
        self.paste_manager.paste_to_active()

    def _on_settings_updated(self):
        """Refresh components after settings change."""
        # Update hotkey
        shortcut = self.db.get_setting('hotkey', 'Ctrl+Shift+V')
        excluded_apps = [a for a in self.db.get_setting('excluded_apps', '').split(',') if a.strip()]
        self.hotkey_manager.update_settings(shortcut, excluded_apps)
        
        # Update autostart
        is_startup = self.db.get_setting('run_on_startup', 'True') == 'True'
        set_autostart(is_startup)
        
        # Update formatting preference in monitor
        is_formatted = self.db.get_setting('preserve_formatting', 'False') == 'True'
        self.clipboard_monitor.preserve_formatting = is_formatted

    def _clear_history(self):
        """Clear the clipboard history but keep master files."""
        reply = QMessageBox.question(None, "Clear History", "Are you sure you want to clear all clipboard history?", 
                                     QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No)
        if reply == QMessageBox.StandardButton.Yes:
            self.db.clear_clipboard_history()
            self.show_main_window()

    def quit_app(self):
        """Cleanup and exit."""
        self.clipboard_monitor.stop()
        self.hotkey_manager.stop()
        self.master_manager.stop()
        self.db.close()
        QCoreApplication.quit()

    def run(self):
        """Start the Qt event loop."""
        return self.app.exec()
