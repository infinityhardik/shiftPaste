# Shift Paste

**Shift Paste** is a high-performance, cross-platform clipboard manager that enhances your productivity with precise fuzzy search and persistent Excel-based collections. It provides a premium, Windows-native feel with intelligent ranking and effortless organization.

## Key Features

✨ **Clipboard History** - Automatically stores your clipboard history with optional text formatting preservation.
🔍 **Sequential Search** - Advanced left-to-right character matching (e.g., search "1884" for "18 mm 8 x 4").
📁 **Master Files** - Keep your frequently used snippets in Excel files; they are automatically synced and ready to search.
⚡ **Instant Access** - Global hotkey (Ctrl+Shift+V) to launch instantly. The UI closes automatically when you paste or lose focus.
🎯 **Smart Ranking** - Results are ranked by a combination of recency and match quality. Consecutive character matches are prioritized.
💾 **Robust Storage** - All history and settings are stored in a lightweight SQLite database (`data/clipboard.db`).
🚫 **App Exclusion** - Disable the hotkey in specific apps (like Photoshop or Excel) to avoid shortcut conflicts.

## Quick Start

### Requirements
- Python 3.10+
- Dependencies: `PySide6`, `pyperclip`, `openpyxl`, `pynput`, `psutil`, `pywin32`

### Installation & Usage
1. **Clone** the repository.
2. **Install dependencies**:
   ```bash
   pip install -r requirements.txt
   ```
3. **Run the application**:
   ```bash
   python main.py
   ```
4. **Hotkey**: Press `Ctrl+Shift+V` to open the search bar. Use arrow keys or type to find an item, then press **Enter** to paste it directly into your active window.

## Search Logic

Shift Paste uses a precise **Left-to-Right Sequential Matching** algorithm:
- It finds every character of your search term in the text, in the order they appear.
- Spaces in your search term are ignored to allow one-string matching.
- **Example**: Search "ro" or "LP" or "1884" to find `18 mm 8 x 4 LL Pro`.
- **Ranking**: A "Match Quality" score is calculated based on the span of the match. Consecutive characters (like "Pro") rank higher than scattered ones (like "1884").

## Maintenance & Configuration

### Data Registry
- **Database**: `data/clipboard.db` stores your history, master items, and settings.
- **Master Files**: Store your Excel sheets in `data/Master/`. Any files added here are automatically scanned and indexed on startup.
- **Polling**: The app polls for changes in your Excel files every 1 second (using modification time), so edits are available almost instantly.

### Settings
Settings are managed entirely via the **Settings Window** in the app. No manual JSON editing is required.
1. Right-click the system tray icon and select **Settings**, or use the gear icon in the search window.
2. You can configure:
   - **Hotkey**: Custom shortcut registration.
   - **History Limit**: Max items to keep in history.
   - **Exclusions**: Add/Remove apps where the hotkey should be disabled.
   - **Master Files**: Manage which Excel sheets are actively searched.

## Architecture

The project has been simplified for maximum performance:
- `src/app.py`: Main application controller and logic.
- `src/core/search_engine.py`: The sequential fuzzy search implementation.
- `src/core/master.py`: Handles Excel indexing and modification polling.
- `src/core/clipboard_monitor.py`: Background thread detecting clipboard changes.
- `src/utils/hotkey.py`: Native Windows API integration for global hotkeys.
- `src/data/database.py`: All storage, settings, and search filtering.

## Troubleshooting

- **Hotkey Fails**: Ensure no other application (like Windows Clipboard) is hogging `Ctrl+Shift+V`. You can change the hotkey in the settings.
- **Excel Not Updating**: Ensure your Excel file is in `.xlsx` format and stored in the `data/Master` folder.
- **App Won't Open**: Check the terminal output. On Windows, ensure `pywin32` and `psutil` are correctly installed.

## Building Executable

You can package Shift Paste into a single `.exe` file using PyInstaller:

1. Install PyInstaller: `pip install pyinstaller`
2. Run the build: `pyinstaller build.spec --clean`
3. Find your app in `dist/ShiftPaste.exe`

For detailed distribution instructions, see [HOW_TO_BUILD.md](file:///c:/Users/Hardik%20Bhaavani/Desktop/shiftPaste/HOW_TO_BUILD.md).

---
**Built for power users who need speed and precision.**
