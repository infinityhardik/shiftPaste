"""Shift Paste - Main entry point.

A high-performance clipboard manager with fuzzy search and Excel master files.

Usage:
    python main.py
    
Or run as module:
    python -m src.app
"""

import sys
from pathlib import Path


def setup_path():
    """Add src directory to Python path for imports."""
    src_path = Path(__file__).parent
    if str(src_path) not in sys.path:
        sys.path.insert(0, str(src_path))


def setup_windows():
    """Configure Windows-specific settings."""
    if sys.platform != 'win32':
        return
        
    try:
        import ctypes
        
        # Set AppUserModelID for proper taskbar icon grouping
        app_id = 'infinityhardik.shiftpaste.1.1'
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(app_id)
        
        # Enable high DPI awareness for crisp display
        try:
            ctypes.windll.shcore.SetProcessDpiAwareness(2)  # Per-monitor DPI aware
        except (AttributeError, OSError):
            try:
                ctypes.windll.user32.SetProcessDPIAware()
            except (AttributeError, OSError):
                pass  # Older Windows version
                
    except Exception as e:
        print(f"[WARN] Could not configure Windows settings: {e}")


def main():
    """Main entry point for Shift Paste."""
    # Setup environment
    setup_path()
    setup_windows()
    
    # Import and run application
    from src.app import ShiftPasteApp
    
    app = ShiftPasteApp()
    sys.exit(app.run())


if __name__ == "__main__":
    main()
