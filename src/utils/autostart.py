"""Auto-start configuration for Shift Paste."""

import os
import sys
import platform

def set_autostart(enabled: bool):
    """Enable or disable auto-start for the application."""
    system = platform.system()
    if system == "Windows":
        _set_windows_autostart(enabled)
    elif system == "Darwin":
        _set_macos_autostart(enabled)
    elif system == "Linux":
        _set_linux_autostart(enabled)

def _set_windows_autostart(enabled: bool):
    """Set Windows autostart via registry."""
    try:
        import winreg
        app_name = "ShiftPaste"
        # If running as script, use python executive, otherwise use sys.executable
        if getattr(sys, 'frozen', False):
            app_path = sys.executable
        else:
            app_path = f'"{sys.executable}" "{os.path.abspath(sys.argv[0])}"'

        key = winreg.HKEY_CURRENT_USER
        key_path = r"Software\Microsoft\Windows\CurrentVersion\Run"
        
        with winreg.OpenKey(key, key_path, 0, winreg.KEY_SET_VALUE) as reg_key:
            if enabled:
                winreg.SetValueEx(reg_key, app_name, 0, winreg.REG_SZ, app_path)
            else:
                try:
                    winreg.DeleteValue(reg_key, app_name)
                except FileNotFoundError:
                    pass
    except Exception as e:
        print(f"Failed to set Windows autostart: {e}")

def _set_macos_autostart(enabled: bool):
    """Placeholder for macOS autostart (using LaunchAgents)."""
    pass

def _set_linux_autostart(enabled: bool):
    """Placeholder for Linux autostart (using .desktop file)."""
    pass
