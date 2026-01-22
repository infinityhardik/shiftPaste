"""Paste mechanism for Shift Paste."""

import platform
import time
from typing import Optional

# Platform-specific imports
try:
    if platform.system() == "Windows":
        import win32gui
        import win32con
    from pynput.keyboard import Controller, Key
except ImportError as e:
    print(f"CRITICAL: Missing dependency: {e}")
    # Define placeholders to avoid NameError
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
    """Manages focus restoration and simulated pasting."""

    def __init__(self):
        self.keyboard = Controller()
        self.last_active_window = None
        self.system = platform.system()

    def capture_active_window(self):
        """Store the currently focused window handle."""
        if self.system == "Windows":
            self.last_active_window = win32gui.GetForegroundWindow()

    def restore_focus(self):
        """Bring the previously active window back to foreground."""
        if self.system == "Windows" and self.last_active_window:
            try:
                if win32gui.IsWindow(self.last_active_window):
                    # Use SW_SHOW to avoid un-maximizing maximized windows
                    win32gui.ShowWindow(self.last_active_window, win32con.SW_SHOW)
                    # Many Windows versions require AttachThreadInput or other tricks for SetForegroundWindow
                    # but usually, since we were triggered by a hotkey, we have permission.
                    try:
                        win32gui.SetForegroundWindow(self.last_active_window)
                    except Exception as focus_err:
                        print(f"Warning: Could not set foreground window: {focus_err}")
                    
                    # Small delay for OS to process focus change
                    time.sleep(0.12)
            except Exception as e:
                print(f"Error restoring focus: {e}")

    def simulate_paste(self):
        """Simulate Ctrl+V or Cmd+V."""
        try:
            if self.system == "Darwin":
                with self.keyboard.pressed(Key.cmd):
                    self.keyboard.press('v')
                    self.keyboard.release('v')
            else:
                with self.keyboard.pressed(Key.ctrl):
                    self.keyboard.press('v')
                    self.keyboard.release('v')
        except Exception as e:
            print(f"Error simulating paste: {e}")

    def paste_to_active(self):
        """Restore focus and then simulate paste."""
        self.restore_focus()
        self.simulate_paste()
