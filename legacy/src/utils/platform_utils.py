"""Platform-specific utilities for process detection.

Used for:
- Identifying the currently active application
- Checking if an app should be excluded from hotkey handling
"""

import platform

# Platform-specific imports with graceful degradation
_HAS_WIN32 = False
_HAS_PSUTIL = False

_system = platform.system()

try:
    if _system == "Windows":
        import win32gui
        import win32process
        _HAS_WIN32 = True
        import psutil
        _HAS_PSUTIL = True
except ImportError as e:
    print(f"[WARN] Platform-specific dependency missing: {e}")


def get_active_process_name() -> str:
    """Get the name of the currently focused process.
    
    Returns:
        Process name (e.g., "notepad.exe") or empty string if unavailable.
        
    Platform support:
        - Windows: Full support via win32 APIs
        - macOS/Linux: Not implemented (returns empty string)
    """
    if _system != "Windows" or not (_HAS_WIN32 and _HAS_PSUTIL):
        return ""
        
    try:
        hwnd = win32gui.GetForegroundWindow()
        if not hwnd:
            return ""
            
        _, pid = win32process.GetWindowThreadProcessId(hwnd)
        if not pid or pid <= 0:
            return ""
            
        process = psutil.Process(pid)
        return process.name()
        
    except psutil.NoSuchProcess:
        # Process terminated between getting PID and querying it
        return ""
    except psutil.AccessDenied:
        # System process or elevated process we can't query
        return ""
    except OSError:
        # Window handle became invalid
        return ""
    except Exception:
        # Catch-all for any unexpected platform errors
        return ""


def is_app_excluded(excluded_list: list) -> bool:
    """Check if the currently active app is in the excluded list.
    
    Used to disable hotkey functionality in certain applications
    (e.g., apps that use the same shortcut for their own features).
    
    Args:
        excluded_list: List of app names to exclude (e.g., ["Photoshop.exe", "Excel.exe"])
        
    Returns:
        True if current app matches any item in excluded_list.
        Case-insensitive substring matching is used.
    """
    if not excluded_list:
        return False
        
    current_app = get_active_process_name()
    if not current_app:
        return False
    
    current_app_lower = current_app.lower()
    
    # Check each excluded app using case-insensitive substring matching
    # This allows "Excel" to match "EXCEL.EXE" or "Microsoft Excel"
    for excluded_app in excluded_list:
        if not excluded_app or not isinstance(excluded_app, str):
            continue
        excluded_lower = excluded_app.strip().lower()
        if excluded_lower and excluded_lower in current_app_lower:
            return True
    
    return False
