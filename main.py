"""Shift Paste - Main entry point"""

import sys
from pathlib import Path

# Add src directory to path
sys.path.insert(0, str(Path(__file__).parent))

from src.app import ShiftPasteApp


def main():
    """Main entry point for Shift Paste."""
    # Set AppUserModelID for Windows Taskbar icon support
    if sys.platform == 'win32':
        import ctypes
        myappid = u'infinityhardik.shiftpaste.1.0'
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(myappid)
        
    app = ShiftPasteApp()
    sys.exit(app.run())


if __name__ == "__main__":
    main()
