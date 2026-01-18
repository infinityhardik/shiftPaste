"""Shift Paste - Main entry point"""

import sys
from pathlib import Path

# Add src directory to path
sys.path.insert(0, str(Path(__file__).parent))

from src.app import ShiftPasteApp


def main():
    """Main entry point for Shift Paste."""
    app = ShiftPasteApp()
    sys.exit(app.run())


if __name__ == "__main__":
    main()
