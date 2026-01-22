"""Clipboard monitoring thread for detecting text changes with security and platform context."""

import time
import hashlib
import platform
import os
from PySide6.QtCore import QThread, Signal
import pyperclip
from typing import Optional, List, Dict

# Platform-specific imports
try:
    if platform.system() == "Windows":
        import win32gui
        import win32process
        import win32clipboard
        import psutil
    elif platform.system() == "Darwin":
        from AppKit import NSPasteboard, NSStringPboardType
except ImportError as e:
    print(f"WARNING: Platform-specific dependency missing: {e}")
    # Define placeholders if needed, but the code already handles missing deps with try-except in methods


EXCLUDED_PROCESSES = {
    'Windows': ['KeePass.exe', '1Password.exe', 'Bitwarden.exe', 'LastPass.exe'],
    'Darwin': ['1Password', 'KeePassXC', 'Bitwarden'],
    'Linux': ['keepassxc', '1password', 'bitwarden']
}


class ClipboardMonitor(QThread):
    """Background thread that monitors clipboard for text changes with process awareness."""

    # Emits (plain_text, is_formatted, formatted_content)
    clipboard_changed = Signal(str, bool, str)

    def __init__(self, poll_interval: float = 0.5):
        """Initialize clipboard monitor."""
        super().__init__()
        self.running = False
        self.poll_interval = poll_interval
        self._last_hash = ""
        self._ignore_next = False
        self.system = platform.system()
        self.preserve_formatting = False # Should be updated from settings

    def get_active_process_name(self) -> str:
        """Get the name of the currently focused process."""
        try:
            if self.system == "Windows":
                hwnd = win32gui.GetForegroundWindow()
                _, pid = win32process.GetWindowThreadProcessId(hwnd)
                process = psutil.Process(pid)
                return process.name()
            # For brevity, standard Linux/Mac implementation omitted or would use platform tools
            return ""
        except Exception:
            return ""

    def is_password_manager_active(self) -> bool:
        """Check if an excluded password manager is currently focused."""
        active_proc = self.get_active_process_name()
        if not active_proc:
            return False
            
        excluded = EXCLUDED_PROCESSES.get(self.system, [])
        return any(pm.lower() in active_proc.lower() for pm in excluded)

    def get_clipboard_data(self) -> tuple[Optional[str], bool, Optional[str]]:
        """Get plain text and optionally formatted text from clipboard."""
        try:
            plain_text = pyperclip.paste()
            if not plain_text:
                return None, False, None
                
            formatted_content = None
            is_formatted = False
            
            if self.preserve_formatting and self.system == "Windows":
                try:
                    win32clipboard.OpenClipboard()
                    # CF_HTML = 49152 (standard but can vary, better use RegisterClipboardFormat)
                    html_format = win32clipboard.RegisterClipboardFormat("HTML Format")
                    if win32clipboard.IsClipboardFormatAvailable(html_format):
                        data = win32clipboard.GetClipboardData(html_format)
                        if isinstance(data, bytes):
                            formatted_content = data.decode('utf-8', errors='ignore')
                            is_formatted = True
                    win32clipboard.CloseClipboard()
                except Exception:
                    try: win32clipboard.CloseClipboard() 
                    except: pass
            
            return plain_text, is_formatted, formatted_content
        except Exception:
            return None, False, None

    def run(self):
        """Main monitoring loop."""
        self.running = True

        while self.running:
            try:
                if not self.is_password_manager_active():
                    plain_text, is_formatted, formatted_content = self.get_clipboard_data()
                    
                    if plain_text:
                        current_hash = hashlib.sha256(plain_text.encode('utf-8')).hexdigest()
                        
                        if current_hash != self._last_hash:
                            if self._ignore_next:
                                self._ignore_next = False
                            else:
                                self.clipboard_changed.emit(plain_text, is_formatted, formatted_content)
                            
                            self._last_hash = current_hash
                
            except Exception as e:
                print(f"Clipboard monitor error: {e}")

            time.sleep(self.poll_interval)

    def stop(self):
        """Stop the monitoring thread."""
        self.running = False

    def ignore_next_change(self):
        """Signal to ignore the next detected change (internal copy)."""
        self._ignore_next = True
