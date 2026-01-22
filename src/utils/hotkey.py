"""Global hotkey management using native Windows API for reliable suppression."""

import ctypes
from ctypes import wintypes
import threading
from typing import Callable, List
from PySide6.QtCore import QObject, Signal
from .platform_utils import is_app_excluded

# Windows Constants
MOD_ALT = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT = 0x0004
MOD_WIN = 0x0008
WM_HOTKEY = 0x0312

class HotkeyManager(QObject):
    """Manages global hotkey registration and suppression via RegisterHotKey."""
    
    triggered = Signal()

    def __init__(self, shortcut_str: str, callback: Callable, excluded_apps: List[str]):
        super().__init__()
        self.shortcut_str = shortcut_str
        self.callback = callback
        self.excluded_apps = excluded_apps
        self.is_listening = False
        self._thread = None
        self._hotkey_id = 1
        self._lock = threading.Lock()
        
        # Mapping for RegisterHotKey
        self.vk_map = {
            'v': 0x56, 'z': 0x5A, 'x': 0x58, 'c': 0x43,
            'backspace': 0x08, 'enter': 0x0D, 'space': 0x20,
        }

    def _parse_shortcut(self, shortcut: str):
        """Parse shortcut string into modifiers and VK code."""
        parts = shortcut.lower().replace(" ", "").split('+')
        modifiers = 0
        vk = 0
        
        for p in parts:
            if p == 'ctrl': modifiers |= MOD_CONTROL
            elif p == 'shift': modifiers |= MOD_SHIFT
            elif p == 'alt': modifiers |= MOD_ALT
            elif p == 'win' or p == 'cmd': modifiers |= MOD_WIN
            elif p in self.vk_map:
                vk = self.vk_map[p]
            elif len(p) == 1:
                vk = ord(p.upper())
                
        return modifiers, vk

    def start(self):
        """Start the hotkey listener thread."""
        with self._lock:
            if not self.is_listening:
                self.is_listening = True
                self._thread = threading.Thread(target=self._run_listener, daemon=True)
                self._thread.start()
                print(f"[*] Hotkey manager started for: {self.shortcut_str}")

    def _run_listener(self):
        """Native message loop for global hotkeys."""
        try:
            modifiers, vk = self._parse_shortcut(self.shortcut_str)
            
            # Register the hotkey
            if not ctypes.windll.user32.RegisterHotKey(None, self._hotkey_id, modifiers, vk):
                print(f"[!] Failed to register hotkey {self.shortcut_str}. Error: {ctypes.GetLastError()}")
                self.is_listening = False
                return

            msg = wintypes.MSG()
            while self.is_listening:
                if ctypes.windll.user32.GetMessageW(ctypes.byref(msg), None, 0, 0) != 0:
                    if msg.message == WM_HOTKEY:
                        try:
                            if not is_app_excluded(self.excluded_apps):
                                self.triggered.emit()
                        except Exception as e:
                            print(f"Error checking excluded apps: {e}")
                    ctypes.windll.user32.TranslateMessage(ctypes.byref(msg))
                    ctypes.windll.user32.DispatchMessageW(ctypes.byref(msg))
        except Exception as e:
            print(f"Hotkey thread error: {e}")
        finally:
            ctypes.windll.user32.UnregisterHotKey(None, self._hotkey_id)
            self.is_listening = False

    def stop(self):
        """Stop the listener and wait for thread to exit."""
        with self._lock:
            if self.is_listening:
                self.is_listening = False
                if self._thread and self._thread.is_alive():
                    # Post a dummy message to break GetMessage
                    ctypes.windll.user32.PostThreadMessageW(self._thread.ident, 0, 0, 0)
                    self._thread.join(timeout=1.0)

    def update_settings(self, new_shortcut: str, new_excluded_apps: List[str]):
        """Update settings and restart listener."""
        self.stop()
        self.shortcut_str = new_shortcut
        self.excluded_apps = new_excluded_apps
        self.start()
