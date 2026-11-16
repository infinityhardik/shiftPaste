"""Paste mechanism for Shift Paste - restores focus and simulates paste.

Security considerations:
- Validates window handles before use
- Uses proper Windows focus restoration sequence
- No clipboard content manipulation (content set before this module is called)

Platform support:
- Windows: Full support with win32 APIs
- macOS: Keyboard simulation only (focus restoration requires Accessibility API)
- Linux: Keyboard simulation only
"""

import platform
import time
from typing import Optional

# Platform-specific imports with graceful fallback
_HAS_WIN32 = False
_HAS_PYNPUT = False

try:
    if platform.system() == "Windows":
        import win32gui
        import win32con
        _HAS_WIN32 = True
except ImportError:
    pass

try:
    from pynput.keyboard import Controller, Key
    _HAS_PYNPUT = True
except ImportError:
    print("[WARN] pynput not installed - paste simulation disabled")
    # Stub classes to prevent NameError
    class Controller:
        def __init__(self): pass
        def pressed(self, key): 
            class Context:
                def __enter__(self): pass
                def __exit__(self, *args): pass
            return Context()
        def press(self, key): pass
        def release(self, key): pass
    
    class Key:
        cmd = ctrl = 'cmd_ctrl'


class PasteManager:
    """Manages focus restoration and simulated paste operations.
    
    Usage:
        1. Call capture_active_window() before showing the app UI
        2. After copying content to clipboard, call paste_to_active()
    
    The paste operation:
        1. Restores focus to the previously active window
        2. Waits briefly for the OS to process the focus change
        3. Simulates Ctrl+V (Windows/Linux) or Cmd+V (macOS)
    """
    
    # Delay after focus restoration before simulating paste
    # Too short = paste may go to wrong window
    # Too long = sluggish UX
    FOCUS_RESTORE_DELAY = 0.10  # 100ms
    
    # Delay between key press and release
    KEY_PRESS_DELAY = 0.02  # 20ms

    def __init__(self):
        self._keyboard = Controller() if _HAS_PYNPUT else None
        self._last_active_window: Optional[int] = None
        self._system = platform.system()

    def capture_active_window(self):
        """Store the currently focused window handle.
        
        Call this before showing any UI that will take focus.
        """
        if self._system == "Windows" and _HAS_WIN32:
            try:
                hwnd = win32gui.GetForegroundWindow()
                if hwnd and win32gui.IsWindow(hwnd):
                    self._last_active_window = hwnd
                else:
                    self._last_active_window = None
            except Exception:
                self._last_active_window = None

    def restore_focus(self) -> bool:
        """Restore focus to the previously active window.
        
        Returns:
            True if focus was successfully restored, False otherwise.
        """
        if self._system != "Windows" or not _HAS_WIN32:
            return False
            
        if not self._last_active_window:
            return False
            
        try:
            hwnd = self._last_active_window
            
            # Validate window still exists
            if not win32gui.IsWindow(hwnd):
                return False
            
            # Restore window if minimized
            # SW_SHOW (5) brings window to foreground without changing size
            win32gui.ShowWindow(hwnd, win32con.SW_SHOW)
            
            # Attempt to set foreground window
            # This may fail if another app has focus lock, but usually works
            # since we were triggered by a user action (hotkey)
            try:
                win32gui.SetForegroundWindow(hwnd)
            except Exception:
                # SetForegroundWindow can fail with permission error
                # Try alternative method: simulate bringing to front
                try:
                    win32gui.BringWindowToTop(hwnd)
                except Exception:
                    return False
            
            # Wait for OS to process focus change
            time.sleep(self.FOCUS_RESTORE_DELAY)
            return True
            
        except Exception as e:
            print(f"[WARN] Could not restore focus: {e}")
            return False

    def simulate_paste(self) -> bool:
        """Simulate Ctrl+V (or Cmd+V on macOS).
        
        Returns:
            True if paste was simulated, False on error.
        """
        if not self._keyboard:
            print("[WARN] Keyboard controller not available")
            return False
            
        try:
            if self._system == "Darwin":
                # macOS: Cmd+V
                with self._keyboard.pressed(Key.cmd):
                    self._keyboard.press('v')
                    time.sleep(self.KEY_PRESS_DELAY)
                    self._keyboard.release('v')
            else:
                # Windows/Linux: Ctrl+V
                with self._keyboard.pressed(Key.ctrl):
                    self._keyboard.press('v')
                    time.sleep(self.KEY_PRESS_DELAY)
                    self._keyboard.release('v')
            return True
            
        except Exception as e:
            print(f"[WARN] Could not simulate paste: {e}")
            return False

    def paste_to_active(self):
        """Restore focus and simulate paste.
        
        This is the main entry point for pasting selected items.
        Even if focus restoration fails, we still attempt the paste
        which may succeed if the expected window is still active.
        """
        self.restore_focus()
        self.simulate_paste()
