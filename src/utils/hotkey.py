"""Global hotkey management using native Windows API for reliable suppression.

Design:
- Uses Windows RegisterHotKey API which suppresses the key from other apps
- Runs in a dedicated thread with its own message loop
- Thread-safe start/stop with proper cleanup

Security:
- Validates excluded apps before triggering callback
- No shell execution or code injection risks
"""

import ctypes
from ctypes import wintypes
import threading
from typing import Callable, List
from PySide6.QtCore import QObject, Signal


# Windows API Constants
MOD_ALT = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT = 0x0004
MOD_WIN = 0x0008
WM_HOTKEY = 0x0312
WM_QUIT = 0x0012


class HotkeyManager(QObject):
    """Manages global hotkey registration using Windows RegisterHotKey API.
    
    The advantage of RegisterHotKey over other approaches:
    - The hotkey is suppressed from reaching other applications
    - Works reliably even when the app is in the background
    - Low CPU usage (no polling)
    """
    
    triggered = Signal()
    
    # Hotkey ID for RegisterHotKey (must be unique per app)
    HOTKEY_ID = 0x0001
    
    # Virtual key code mappings
    VK_MAP = {
        'backspace': 0x08,
        'tab': 0x09,
        'enter': 0x0D,
        'return': 0x0D,
        'space': 0x20,
        'end': 0x23,
        'home': 0x24,
        'left': 0x25,
        'up': 0x26,
        'right': 0x27,
        'down': 0x28,
        'delete': 0x2E,
        # Add alphanumeric - handled dynamically
        **{chr(i): ord(chr(i).upper()) for i in range(ord('a'), ord('z') + 1)},
        **{str(i): ord(str(i)) for i in range(10)},
    }

    def __init__(self, shortcut_str: str, callback: Callable, excluded_apps: List[str]):
        """Initialize hotkey manager.
        
        Args:
            shortcut_str: Hotkey string like "Ctrl+Shift+V"
            callback: Function to call when hotkey is pressed
            excluded_apps: List of app names where hotkey should be ignored
        """
        super().__init__()
        self._shortcut_str = shortcut_str.strip()
        self._callback = callback
        self._excluded_apps = list(excluded_apps)
        self._is_listening = False
        self._thread: threading.Thread = None
        self._thread_id: int = 0
        self._lock = threading.Lock()

    def _parse_shortcut(self, shortcut: str) -> tuple:
        """Parse shortcut string into modifiers and VK code.
        
        Args:
            shortcut: String like "Ctrl+Shift+V"
            
        Returns:
            Tuple of (modifiers: int, vk_code: int)
        """
        parts = shortcut.lower().replace(" ", "").split('+')
        modifiers = 0
        vk = 0
        
        for part in parts:
            if part in ('ctrl', 'control'):
                modifiers |= MOD_CONTROL
            elif part == 'shift':
                modifiers |= MOD_SHIFT
            elif part == 'alt':
                modifiers |= MOD_ALT
            elif part in ('win', 'cmd', 'super', 'meta'):
                modifiers |= MOD_WIN
            elif part in self.VK_MAP:
                vk = self.VK_MAP[part]
            elif len(part) == 1 and part.isalnum():
                vk = ord(part.upper())
                
        return modifiers, vk

    def start(self):
        """Start the hotkey listener thread."""
        with self._lock:
            if self._is_listening:
                return
                
            self._is_listening = True
            self._thread = threading.Thread(target=self._run_listener, daemon=True)
            self._thread.start()
            print(f"[*] Hotkey manager started: {self._shortcut_str}")

    def _run_listener(self):
        """Native message loop for global hotkeys.
        
        Runs in a dedicated thread with its own Windows message loop.
        """
        try:
            self._thread_id = threading.current_thread().ident
            modifiers, vk = self._parse_shortcut(self._shortcut_str)
            
            if vk == 0:
                print(f"[!] Invalid hotkey configuration: {self._shortcut_str}")
                self._is_listening = False
                return
            
            # Register the hotkey
            result = ctypes.windll.user32.RegisterHotKey(None, self.HOTKEY_ID, modifiers, vk)
            if not result:
                error = ctypes.GetLastError()
                print(f"[!] Failed to register hotkey '{self._shortcut_str}' (error: {error})")
                print("[!] Another application may have registered the same hotkey")
                self._is_listening = False
                return

            # Message loop
            msg = wintypes.MSG()
            while self._is_listening:
                result = ctypes.windll.user32.GetMessageW(ctypes.byref(msg), None, 0, 0)
                
                if result <= 0:  # WM_QUIT or error
                    break
                    
                if msg.message == WM_HOTKEY:
                    self._handle_hotkey()
                    
                ctypes.windll.user32.TranslateMessage(ctypes.byref(msg))
                ctypes.windll.user32.DispatchMessageW(ctypes.byref(msg))
                    
        except Exception as e:
            print(f"[ERROR] Hotkey thread error: {e}")
        finally:
            # Cleanup
            ctypes.windll.user32.UnregisterHotKey(None, self.HOTKEY_ID)
            self._is_listening = False
            self._thread_id = 0

    def _handle_hotkey(self):
        """Handle hotkey press - check exclusions and emit signal."""
        try:
            # Import here to avoid circular dependency
            from .platform_utils import is_app_excluded
            
            if not is_app_excluded(self._excluded_apps):
                self.triggered.emit()
                
        except Exception as e:
            print(f"[WARN] Error in hotkey handler: {e}")

    def stop(self):
        """Stop the listener and wait for thread to exit."""
        with self._lock:
            if not self._is_listening:
                return
                
            self._is_listening = False
            
            if self._thread_id and self._thread and self._thread.is_alive():
                # Post WM_QUIT to break the message loop
                try:
                    ctypes.windll.user32.PostThreadMessageW(
                        self._thread_id, WM_QUIT, 0, 0
                    )
                except Exception:
                    pass
                    
                # Wait for thread to exit with timeout
                self._thread.join(timeout=2.0)
                
                if self._thread.is_alive():
                    print("[WARN] Hotkey thread did not stop gracefully")

    def update_settings(self, new_shortcut: str, new_excluded_apps: List[str]):
        """Update settings and restart listener.
        
        Args:
            new_shortcut: New hotkey string
            new_excluded_apps: Updated list of excluded apps
        """
        self.stop()
        self._shortcut_str = new_shortcut.strip()
        self._excluded_apps = list(new_excluded_apps)
        self.start()

    def __del__(self):
        """Destructor to ensure cleanup."""
        self.stop()
