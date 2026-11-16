"""Path resolution utilities for portable and installed deployments.

This module provides consistent path resolution that works correctly for:
- Development mode (running from source)
- PyInstaller frozen executable (dist/ShiftPaste.exe)
- Inno Setup installed application (C:\Program Files\Shift Paste)

Key principles:
- Resources (icons, bundled data) come from the app directory
- User data (database, settings) goes to a writable location
- On Windows, user data is stored in %LOCALAPPDATA%
"""

import sys
import os
from pathlib import Path


def is_frozen() -> bool:
    """Check if running as a PyInstaller frozen executable.
    
    Returns:
        True if running from frozen executable (PyInstaller bundle).
    """
    return getattr(sys, 'frozen', False) and hasattr(sys, '_MEIPASS')


def get_app_dir() -> Path:
    """Get the application directory (where the exe/script is located).
    
    For frozen apps: directory containing the exe
    For development: project root (parent of src)
    
    Returns:
        Path to application directory.
    """
    if is_frozen():
        # Frozen executable - use directory containing the exe
        return Path(sys.executable).parent
    else:
        # Development - go up from src/utils/paths.py to project root
        return Path(__file__).parent.parent.parent


def get_resource_dir() -> Path:
    """Get the directory containing bundled resources.
    
    For frozen apps: _MEIPASS temp directory (bundled resources)
    For installed apps: may fall back to app_dir/resources
    For development: project root
    
    Returns:
        Path to resources directory.
    """
    if is_frozen():
        # First check the _MEIPASS bundled resources
        meipass = Path(sys._MEIPASS)
        resources_in_bundle = meipass / 'resources'
        if resources_in_bundle.exists():
            return meipass
        
        # Fall back to resources next to the exe (Inno Setup copies them there)
        app_resources = get_app_dir() / 'resources'
        if app_resources.exists():
            return get_app_dir()
        
        # Last resort: use _MEIPASS directly
        return meipass
    else:
        return get_app_dir()


def get_user_data_dir() -> Path:
    """Get the directory for user data (database, settings, etc.).
    
    User data must be in a writable location that persists.
    - Windows: %LOCALAPPDATA%/ShiftPaste
    - macOS: ~/Library/Application Support/ShiftPaste
    - Linux: ~/.local/share/ShiftPaste
    
    Returns:
        Path to user data directory (created if needed).
    """
    if sys.platform == 'win32':
        # Windows: Use LOCALAPPDATA for per-user, machine-local data
        local_app_data = os.environ.get('LOCALAPPDATA')
        if local_app_data:
            data_dir = Path(local_app_data) / 'ShiftPaste'
        else:
            # Fallback to user home
            data_dir = Path.home() / 'AppData' / 'Local' / 'ShiftPaste'
    elif sys.platform == 'darwin':
        # macOS
        data_dir = Path.home() / 'Library' / 'Application Support' / 'ShiftPaste'
    else:
        # Linux and other Unix
        xdg_data = os.environ.get('XDG_DATA_HOME')
        if xdg_data:
            data_dir = Path(xdg_data) / 'ShiftPaste'
        else:
            data_dir = Path.home() / '.local' / 'share' / 'ShiftPaste'
    
    # Ensure directory exists
    try:
        data_dir.mkdir(parents=True, exist_ok=True)
    except OSError as e:
        print(f"[WARN] Could not create data directory: {e}")
        # Fallback to current directory as last resort
        data_dir = Path.cwd()
    
    return data_dir


def get_database_path() -> Path:
    """Get the path to the clipboard database.
    
    Returns:
        Path to clipboard.db file.
    """
    return get_user_data_dir() / 'clipboard.db'


def get_master_files_dir() -> Path:
    """Get the directory for master Excel files.
    
    Returns:
        Path to master files directory (created if needed).
    """
    master_dir = get_user_data_dir() / 'Master'
    try:
        master_dir.mkdir(parents=True, exist_ok=True)
    except OSError:
        pass
    return master_dir


def get_icon_path(icon_name: str = 'app_icon.ico') -> Path:
    """Get the path to an icon file.
    
    Args:
        icon_name: Name of the icon file.
        
    Returns:
        Path to the icon file.
    """
    # Try multiple locations for icons
    possible_paths = [
        get_resource_dir() / 'resources' / 'icons' / icon_name,
        get_app_dir() / 'resources' / 'icons' / icon_name,
    ]
    
    if is_frozen():
        # Also check _MEIPASS directly
        possible_paths.insert(0, Path(sys._MEIPASS) / 'resources' / 'icons' / icon_name)
    
    for path in possible_paths:
        if path.exists():
            return path
    
    # Return first possibility even if it doesn't exist
    return possible_paths[0]


def migrate_old_data():
    """Migrate data from old locations to the new user data directory.
    
    This handles the transition from storing data relative to the exe
    to storing it in the proper user data location.
    """
    import shutil
    
    new_db_path = get_database_path()
    new_master_dir = get_master_files_dir()
    
    # Skip if new database already exists
    if new_db_path.exists():
        return
    
    # Check for old database locations
    old_locations = [
        get_app_dir() / 'data' / 'clipboard.db',
        Path.cwd() / 'data' / 'clipboard.db',
    ]
    
    for old_db in old_locations:
        if old_db.exists():
            try:
                print(f"[*] Migrating database from {old_db} to {new_db_path}")
                shutil.copy2(old_db, new_db_path)
                
                # Also migrate Master files if they exist
                old_master_dir = old_db.parent / 'Master'
                if old_master_dir.exists() and old_master_dir.is_dir():
                    for file in old_master_dir.iterdir():
                        dest = new_master_dir / file.name
                        if not dest.exists():
                            shutil.copy2(file, dest)
                            print(f"[*] Migrated master file: {file.name}")
                
                print("[*] Migration complete!")
                return
            except Exception as e:
                print(f"[WARN] Could not migrate old data: {e}")


# Debug output when module loads (helps troubleshoot path issues)
if __name__ == '__main__':
    print(f"Frozen: {is_frozen()}")
    print(f"App dir: {get_app_dir()}")
    print(f"Resource dir: {get_resource_dir()}")
    print(f"User data dir: {get_user_data_dir()}")
    print(f"Database path: {get_database_path()}")
    print(f"Master files dir: {get_master_files_dir()}")
    print(f"Icon path: {get_icon_path()}")
