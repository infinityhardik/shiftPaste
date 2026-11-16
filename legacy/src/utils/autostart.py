"""Auto-start configuration for Shift Paste.

Platform support:
- Windows: Registry Run key (HKCU)
- macOS: Launch Agents (placeholder)
- Linux: XDG autostart .desktop files (placeholder)

Security:
- Uses user-level registry key (no admin required)
- Properly escapes paths with spaces
"""

import os
import sys
import platform
from typing import Optional


def set_autostart(enabled: bool) -> bool:
    """Enable or disable auto-start for the application.
    
    Args:
        enabled: True to enable autostart, False to disable
        
    Returns:
        True if operation succeeded, False otherwise
    """
    system = platform.system()
    
    if system == "Windows":
        return _set_windows_autostart(enabled)
    elif system == "Darwin":
        return _set_macos_autostart(enabled)
    elif system == "Linux":
        return _set_linux_autostart(enabled)
    else:
        print(f"[WARN] Autostart not supported on {system}")
        return False


def _get_app_path() -> str:
    """Get the command to launch this application.
    
    Handles both frozen (PyInstaller) and script modes.
    
    Returns:
        Command string to launch the application
    """
    if getattr(sys, 'frozen', False):
        # Running as compiled executable
        return f'"{sys.executable}"'
    else:
        # Running as script
        main_script = os.path.abspath(sys.argv[0])
        python_exe = sys.executable
        return f'"{python_exe}" "{main_script}"'


def _set_windows_autostart(enabled: bool) -> bool:
    """Set Windows autostart via registry Run key.
    
    Uses HKEY_CURRENT_USER so no admin privileges are required.
    
    Args:
        enabled: True to enable, False to disable
        
    Returns:
        True if operation succeeded
    """
    try:
        import winreg
    except ImportError:
        print("[WARN] winreg not available - cannot set autostart")
        return False
        
    app_name = "ShiftPaste"
    key_path = r"Software\Microsoft\Windows\CurrentVersion\Run"
    
    try:
        key = winreg.OpenKey(
            winreg.HKEY_CURRENT_USER, 
            key_path, 
            0, 
            winreg.KEY_SET_VALUE | winreg.KEY_QUERY_VALUE
        )
        
        try:
            if enabled:
                app_path = _get_app_path()
                winreg.SetValueEx(key, app_name, 0, winreg.REG_SZ, app_path)
                print(f"[*] Autostart enabled: {app_path}")
            else:
                try:
                    winreg.DeleteValue(key, app_name)
                    print("[*] Autostart disabled")
                except FileNotFoundError:
                    # Value doesn't exist - that's fine
                    pass
            return True
        finally:
            winreg.CloseKey(key)
            
    except PermissionError:
        print("[ERROR] Permission denied when setting autostart")
        return False
    except Exception as e:
        print(f"[ERROR] Failed to set Windows autostart: {e}")
        return False


def _set_macos_autostart(enabled: bool) -> bool:
    """Set macOS autostart via Launch Agents.
    
    Creates a plist file in ~/Library/LaunchAgents/
    
    TODO: Implement for macOS support
    
    Args:
        enabled: True to enable, False to disable
        
    Returns:
        True if operation succeeded
    """
    # Placeholder for macOS implementation
    # Would create/remove ~/Library/LaunchAgents/com.shiftpaste.plist
    print("[WARN] macOS autostart not yet implemented")
    return False


def _set_linux_autostart(enabled: bool) -> bool:
    """Set Linux autostart via XDG autostart.
    
    Creates a .desktop file in ~/.config/autostart/
    
    TODO: Implement for Linux support
    
    Args:
        enabled: True to enable, False to disable
        
    Returns:
        True if operation succeeded
    """
    # Placeholder for Linux implementation
    # Would create/remove ~/.config/autostart/shiftpaste.desktop
    print("[WARN] Linux autostart not yet implemented")
    return False


def is_autostart_enabled() -> Optional[bool]:
    """Check if autostart is currently enabled.
    
    Returns:
        True if enabled, False if disabled, None if unknown/error
    """
    system = platform.system()
    
    if system == "Windows":
        try:
            import winreg
            key = winreg.OpenKey(
                winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                0,
                winreg.KEY_QUERY_VALUE
            )
            try:
                winreg.QueryValueEx(key, "ShiftPaste")
                return True
            except FileNotFoundError:
                return False
            finally:
                winreg.CloseKey(key)
        except Exception:
            return None
    
    return None
