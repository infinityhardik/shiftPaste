"""Main application controller for Shift Paste.

This module orchestrates all components:
- Database for storage
- Clipboard monitoring
- Search engine
- Master file management
- UI windows
- System tray
- Global hotkeys

Architecture:
- Single-instance application
- QThread-based background tasks
- Signal/slot communication between components
"""

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
    """Main application controller.
    
    Coordinates all application components and handles lifecycle.
    """
    
    # Application metadata
    APP_NAME = "Shift Paste"
    APP_VERSION = "1.1.0"
    ORG_NAME = "infinityhardik"

    def __init__(self):
        """Initialize application and its components."""
        super().__init__()
        
        # Component references (initialized in order)
        self.app: Optional[QApplication] = None
        self.db: Optional[Database] = None
        self.search_engine: Optional[FuzzySearchEngine] = None
        self.paste_manager: Optional[PasteManager] = None
        self.master_manager: Optional[MasterManager] = None
        self.clipboard_monitor: Optional[ClipboardMonitor] = None
        self.main_window: Optional[MainWindow] = None
        self.settings_window: Optional[SettingsWindow] = None
        self.tray_manager: Optional[SystemTrayManager] = None
        self.hotkey_manager: Optional[HotkeyManager] = None
        
        self._initialize()

    def _initialize(self):
        """Initialize all application components in order."""
        try:
            # 1. Initialize Qt Application
            print(f"[*] Starting {self.APP_NAME} v{self.APP_VERSION}...")
            self.app = QApplication(sys.argv)
            self.app.setApplicationName(self.APP_NAME)
            self.app.setOrganizationName(self.ORG_NAME)
            self.app.setQuitOnLastWindowClosed(False)  # Keep running in tray
            
            # 2. Initialize Database
            print("[*] Initializing database...")
            self.db = Database()
            
            # 3. Initialize core components
            print("[*] Initializing core components...")
            self.search_engine = FuzzySearchEngine()
            self.paste_manager = PasteManager()
            self.master_manager = MasterManager(self.db)
            
            # 4. Start clipboard monitor
            print("[*] Starting clipboard monitor...")
            self._init_clipboard_monitor()
            
            # 5. Initialize UI
            print("[*] Initializing UI...")
            self._init_ui()
            
            # 6. Initialize system tray
            print("[*] Setting up system tray...")
            self._init_tray()
            
            # 7. Register hotkeys
            print("[*] Registering hotkeys...")
            self._init_hotkeys()
            
            # 8. Apply initial settings
            self._apply_settings()
            
            print(f"[*] {self.APP_NAME} is ready!")
            
        except Exception as e:
            print(f"[ERROR] Critical error during startup: {e}")
            import traceback
            traceback.print_exc()
            self._emergency_cleanup()
            sys.exit(1)

    def _init_clipboard_monitor(self):
        """Initialize and start the clipboard monitor."""
        # Get formatting preference
        preserve_fmt = self.db.get_setting('preserve_formatting', 'False') == 'True'
        
        self.clipboard_monitor = ClipboardMonitor()
        self.clipboard_monitor.preserve_formatting = preserve_fmt
        self.clipboard_monitor.clipboard_changed.connect(
            self._on_clipboard_changed,
            Qt.ConnectionType.QueuedConnection  # Thread-safe
        )
        self.clipboard_monitor.start()

    def _init_ui(self):
        """Initialize UI windows."""
        self.main_window = MainWindow()
        self.main_window.search_changed.connect(self._on_search_changed)
        self.main_window.item_selected.connect(self._on_item_selected)
        self.main_window.settings_requested.connect(self.show_settings)
        self.main_window.clear_requested.connect(self._clear_history)

    def _init_tray(self):
        """Initialize system tray icon and menu."""
        self.tray_manager = SystemTrayManager()
        self.tray_manager.show_requested.connect(self.show_main_window)
        self.tray_manager.settings_requested.connect(self.show_settings)
        self.tray_manager.clear_requested.connect(self._clear_history)
        self.tray_manager.exit_requested.connect(self.quit_app)

    def _init_hotkeys(self):
        """Initialize global hotkey handling."""
        shortcut = self.db.get_setting('hotkey', 'Ctrl+Shift+V')
        excluded_apps_str = self.db.get_setting('excluded_apps', '')
        excluded_apps = [a.strip() for a in excluded_apps_str.split(',') if a.strip()]
        
        self.hotkey_manager = HotkeyManager(shortcut, self.toggle_main_window, excluded_apps)
        self.hotkey_manager.triggered.connect(
            self.toggle_main_window,
            Qt.ConnectionType.QueuedConnection  # Thread-safe
        )
        self.hotkey_manager.start()

    def _apply_settings(self):
        """Apply settings from database to components."""
        # Autostart
        autostart = self.db.get_setting('run_on_startup', 'True') == 'True'
        set_autostart(autostart)

    def show_main_window(self):
        """Display the main popup near the cursor."""
        # Capture the window that was active before showing our UI
        self.paste_manager.capture_active_window()
        
        # Load recent items for initial display
        items = self.db.get_recent_items(20)
        
        # Add time ago strings
        for item in items:
            ts = item.get('last_copied_at') or item.get('master_modified')
            item['time_ago'] = self.search_engine.get_time_ago_string(ts)
        
        self.main_window.update_results(items)
        self.main_window.show_near_cursor()

    def toggle_main_window(self):
        """Toggle the main window visibility."""
        if self.main_window.isVisible():
            self.main_window.close()
        else:
            self.show_main_window()

    def show_settings(self):
        """Open the settings dialog."""
        # Close main window when opening settings
        if self.main_window.isVisible():
            self.main_window.close()
        
        # Create new settings window
        self.settings_window = SettingsWindow(self.db, self.master_manager)
        self.settings_window.settings_changed.connect(self._on_settings_changed)
        
        # Show and bring to front
        self.settings_window.show()
        self.settings_window.raise_()
        self.settings_window.activateWindow()

    def _on_clipboard_changed(self, text: str, is_formatted: bool, formatted_content: Optional[str]):
        """Handle new clipboard content detected by monitor."""
        try:
            if self.db and self.db.conn:
                self.db.add_clipboard_item(text, is_formatted, formatted_content)
        except Exception as e:
            print(f"[WARN] Error saving clipboard item: {e}")

    def _on_search_changed(self, query: str):
        """Execute search and update UI."""
        if not query.strip():
            # Empty query - show recent items
            items = self.db.get_recent_items(20)
        else:
            # Search both clipboard and masters
            clip_results = self.db.search_clipboard(query, limit=50)
            master_results = self.db.search_masters(query, limit=50)
            
            # Merge and rank results
            all_results = clip_results + master_results
            items = self.search_engine.rank_search_results(all_results, query)
            items = items[:20]  # Limit displayed results

        # Add time ago strings
        for item in items:
            ts = item.get('last_copied_at') or item.get('master_modified')
            item['time_ago'] = self.search_engine.get_time_ago_string(ts)

        self.main_window.update_results(items)

    def _on_item_selected(self, item: dict):
        """Handle item selection - copy to clipboard and paste."""
        content = item.get('content', '')
        if not content:
            return
        
        # Ignore the next clipboard change (it's our own copy)
        self.clipboard_monitor.ignore_next_change()
        
        # Copy content to clipboard
        try:
            import pyperclip
            pyperclip.copy(content)
        except Exception as e:
            print(f"[WARN] Could not copy to clipboard: {e}")
            return
        
        # Paste to active window
        self.paste_manager.paste_to_active()

    def _on_settings_changed(self):
        """Handle settings changes - update components."""
        print("[*] Settings updated, applying changes...")
        
        # Update hotkey
        shortcut = self.db.get_setting('hotkey', 'Ctrl+Shift+V')
        excluded_apps_str = self.db.get_setting('excluded_apps', '')
        excluded_apps = [a.strip() for a in excluded_apps_str.split(',') if a.strip()]
        self.hotkey_manager.update_settings(shortcut, excluded_apps)
        
        # Update autostart
        autostart = self.db.get_setting('run_on_startup', 'True') == 'True'
        set_autostart(autostart)
        
        # Update clipboard monitor formatting preference
        preserve_fmt = self.db.get_setting('preserve_formatting', 'False') == 'True'
        self.clipboard_monitor.preserve_formatting = preserve_fmt

    def _clear_history(self):
        """Clear clipboard history after confirmation."""
        # Find active parent to ensure it stays on top
        parent = self.main_window if self.main_window.isVisible() else None
        
        reply = QMessageBox.question(
            parent, 
            "Clear History",
            "Are you sure you want to clear all clipboard history?\n\n"
            "This action cannot be undone.",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No
        )
        
        if reply == QMessageBox.StandardButton.Yes:
            self.db.clear_clipboard_history()
            # Refresh the main window if visible
            if self.main_window.isVisible():
                self.show_main_window()

    def quit_app(self):
        """Cleanup and exit the application gracefully."""
        print("[*] Shutting down...")
        
        # Stop background threads
        if self.clipboard_monitor:
            print("[*] Stopping clipboard monitor...")
            self.clipboard_monitor.stop()
        
        if self.hotkey_manager:
            print("[*] Stopping hotkey manager...")
            self.hotkey_manager.stop()
        
        if self.master_manager:
            print("[*] Stopping master manager...")
            self.master_manager.stop()
        
        # Close database
        if self.db:
            print("[*] Closing database...")
            self.db.close()
        
        # Hide tray icon
        if self.tray_manager:
            self.tray_manager.hide()
        
        print("[*] Goodbye!")
        QCoreApplication.quit()

    def _emergency_cleanup(self):
        """Minimal cleanup for crash situations."""
        try:
            if self.clipboard_monitor:
                self.clipboard_monitor.stop()
        except:
            pass
        try:
            if self.hotkey_manager:
                self.hotkey_manager.stop()
        except:
            pass
        try:
            if self.master_manager:
                self.master_manager.stop()
        except:
            pass
        try:
            if self.db:
                self.db.close()
        except:
            pass

    def run(self) -> int:
        """Start the Qt event loop.
        
        Returns:
            Exit code (0 for success)
        """
        return self.app.exec()
