"""Platform-specific utilities."""

import platform
import os

# Platform-specific imports
try:
    if platform.system() == "Windows":
        import win32gui
        import win32process
        import psutil
except ImportError as e:
    print(f"WARNING: Missing platform-specific dependency in platform_utils: {e}")

def get_active_process_name() -> str:
    """Get the name of the currently focused process."""
    system = platform.system()
    try:
        if system == "Windows":
            hwnd = win32gui.GetForegroundWindow()
            _, pid = win32process.GetWindowThreadProcessId(hwnd)
            process = psutil.Process(pid)
            return process.name()
        # macOS/Linux would need additional dependencies or applescript
        return ""
    except Exception:
        return ""

def is_app_excluded(excluded_list: list) -> bool:
    """Check if the currently active app is in the excluded list."""
    current_app = get_active_process_name()
    if not current_app:
        return False
    return any(app.lower() in current_app.lower() for app in excluded_list if app.strip())
