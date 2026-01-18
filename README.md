# Shift Paste

**Shift Paste** is an open-source, cross-platform clipboard manager that enhances the native clipboard experience with powerful fuzzy search capabilities and persistent master files. It feels identical to Windows 10 Clipboard History but adds intelligent search and Excel-based organization.

## Features

✨ **Clipboard History** - Automatically stores your clipboard items
🔍 **Fuzzy Search** - Intelligent left-to-right character matching
📁 **Master Files** - Persistent Excel-based collections (Pinned, Work, Personal)
⚡ **Instant Access** - Global hotkey (Ctrl+Shift+V) for quick access
🎯 **Ranking** - Results ranked by match quality and recency
💾 **Local Storage** - SQLite database with full-text search
🌍 **Cross-Platform** - Works on Windows, macOS, and Linux

## Quick Start

### Requirements

- Python 3.10 or higher
- PySide6, pyperclip, keyboard, openpyxl, pandas, watchdog, pyautogui

### Installation

1. **Clone or download** the repository
2. **Install dependencies**:
   ```bash
   pip install -r requirements.txt
   ```

3. **Run the application**:
   ```bash
   python main.py
   ```

The app will start in the system tray. Press **Ctrl+Shift+V** to open the clipboard manager.

## Usage

### Opening Shift Paste

- Press `Ctrl+Shift+V` (Windows)
- Press `Shift+Cmd+V` (macOS)
- Press `Shift+Super+V` (Linux)

### Finding Items

1. Type to search
2. Results update in real-time
3. Press **Enter** or **double-click** to paste
4. Press **Delete** to remove a clipboard item
5. Press **Esc** to close

### Master Files

Master files are Excel spreadsheets stored in `data/Master/`:

- **Pinned.xlsx** - Frequently used items
- **Work.xlsx** - Work-related templates/snippets
- **Personal.xlsx** - Personal snippets

Edit these files directly, and the app will reload them automatically.

## Configuration

Settings are stored in `config.json`:

```json
{
  "clipboard": {
    "max_items": 20,
    "preview_chars": 100
  },
  "shortcuts": {
    "windows": "ctrl+shift+v",
    "macos": "shift+cmd+v",
    "linux": "shift+super+v"
  },
  "master_file": {
    "directory": "data/Master",
    "auto_reload": true
  },
  "ui": {
    "theme": "system",
    "max_visible_items": 8
  },
  "startup": {
    "run_on_boot": false
  }
}
```

## Architecture

```
src/
├── app.py              # Main application controller
├── ui/
│   ├── main_window.py      # Main popup window
│   ├── settings_window.py  # Settings dialog
│   └── styles.py           # QSS stylesheets
├── core/
│   ├── clipboard_monitor.py # Clipboard change detection
│   ├── search_engine.py    # Fuzzy search algorithm
│   └── hotkey_manager.py   # Global hotkey handling
├── data/
│   ├── database.py         # SQLite operations
│   ├── excel_manager.py    # Excel file handling
│   └── config_manager.py   # Configuration management
└── utils/
    └── platform_utils.py   # OS-specific utilities
```

## Fuzzy Search Algorithm

Shift Paste uses left-to-right character matching:

**Valid searches:**
- `MARLEX A Grade` → "MARLEX A Grade 100%" ✓
- `mrlx` → M-A-R-L-EX ✓
- `door` → "...Flush Door" ✓

**Invalid searches:**
- `xml` → "No left-to-right match" ✗
- `door marlex` → "Wrong order" ✗

Results are ranked by:
1. **Match quality** (60%) - Consecutive characters, word boundaries
2. **Recency** (40%) - Newer items ranked higher
3. **Master boost** (1.1x) - Master items get priority

## Database

SQLite database with:
- `clipboard_items` - Recent clipboard history
- `master_items` - Persistent master items
- `search_index` - FTS5 full-text search index

## Building Executable

Create a standalone executable using PyInstaller:

```bash
pyinstaller build.spec --clean
```

Output:
- Windows: `ShiftPaste.exe` (~60MB)
- macOS: `ShiftPaste.app` (~65MB)
- Linux: `ShiftPaste` (~60MB)

## Troubleshooting

### Hotkey not working
- Check if another app is using the same hotkey
- Change hotkey in Settings
- Ensure application has administrator privileges (Windows)

### Items not appearing
- Check clipboard is working (Ctrl+C some text)
- Verify `data/clipboard.db` exists
- Clear history and try again

### Excel files not syncing
- Ensure files are in `data/Master/` directory
- Check file has proper headers: Content, Timestamp, Notes
- Verify auto-reload is enabled in Settings

## License

MIT License - See LICENSE file for details

## Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## Support

For issues or questions:
1. Check existing GitHub issues
2. Create a new issue with details
3. Include steps to reproduce

---

**Built with ❤️ for productivity**
