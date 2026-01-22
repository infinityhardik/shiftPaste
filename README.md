# Shift Paste

> **A high-performance clipboard manager with fuzzy search and Excel-based master collections.**

Shift Paste enhances your productivity with intelligent clipboard history, precise sequential search, and persistent snippet libraries stored in Excel files. Built with a Windows-native feel and optimized for power users.

![Python 3.10+](https://img.shields.io/badge/Python-3.10+-blue.svg)
![PySide6](https://img.shields.io/badge/UI-PySide6-green.svg)
![Windows](https://img.shields.io/badge/Platform-Windows-blue.svg)

---

## ✨ Key Features

| Feature | Description |
|---------|-------------|
| 📋 **Clipboard History** | Automatically captures and stores your clipboard history with deduplication |
| 🔍 **Sequential Search** | Left-to-right character matching (search "1884" to find "18 mm 8 x 4") |
| 📁 **Master Files** | Keep frequently used snippets in Excel files, automatically indexed |
| ⚡ **Instant Access** | Global hotkey (`Ctrl+Shift+V`) launches instantly near your cursor |
| 🎯 **Smart Ranking** | Results ranked by recency + match quality, with consecutive matches prioritized |
| 🔒 **Security-Aware** | Automatically excludes password manager content from history |
| 🚫 **App Exclusion** | Disable the hotkey in specific apps (Photoshop, Excel, etc.) |

---

## 🚀 Quick Start

### Requirements
- **Python 3.10+** (tested with 3.11, 3.12, 3.13)
- **Windows 10/11** (primary platform)

### Installation

```bash
# Clone the repository
git clone https://github.com/infinityhardik/shiftPaste.git
cd shiftPaste

# Install dependencies
pip install -r requirements.txt

# Run the application
python main.py
```

### Usage

1. **Press `Ctrl+Shift+V`** to open the clipboard manager
2. **Type to search** - characters are matched in order across the entire text
3. **Arrow keys** to navigate, **Enter** to paste
4. **Escape** or click outside to close

---

## 🔍 Search Algorithm

Shift Paste uses **Left-to-Right Sequential Matching**:

- Characters are found in order, but can be separated by any text
- Spaces in your search are ignored
- Consecutive matches rank higher than scattered ones

**Examples:**
| Search | Matches |
|--------|---------|
| `ro` | "18 mm P**ro** Model" |
| `LP` | "18 mm 8 x 4 L**L** **P**ro" |
| `1884` | "**18** mm **8** x **4** LL Pro" |

---

## ⚙️ Configuration

### Settings Window
Right-click the system tray icon → **Settings**, or press `Ctrl+,` in the popup.

Available options:
- **Hotkey**: Custom keyboard shortcut
- **History Limit**: 25 / 50 / 100 / 200 / 500 / Unlimited
- **Master Files**: Add/remove Excel files for snippet search
- **App Exclusions**: List of applications where hotkey is disabled
- **Startup**: Run on Windows login
- **Security**: Exclude password manager clipboard content

### Data Storage
- **Database**: `data/clipboard.db` (SQLite)
- **Master Files**: Place Excel files in `data/Master/` for automatic indexing

---

## 🏗️ Architecture

```
shiftpaste/
├── main.py                 # Entry point
├── src/
│   ├── app.py              # Main application controller
│   ├── core/
│   │   ├── clipboard_monitor.py  # Background clipboard watcher
│   │   ├── search_engine.py      # Fuzzy search implementation
│   │   ├── master.py             # Excel file indexing
│   │   └── paste.py              # Focus restoration & paste
│   ├── data/
│   │   └── database.py           # SQLite storage layer
│   ├── ui/
│   │   ├── main_window.py        # Popup window
│   │   ├── settings_window.py    # Settings dialog
│   │   ├── tray.py               # System tray icon
│   │   └── styles.py             # QSS stylesheets
│   └── utils/
│       ├── hotkey.py             # Global hotkey via Win32 API
│       ├── autostart.py          # Windows startup registration
│       └── platform_utils.py     # Active process detection
├── data/
│   ├── clipboard.db              # SQLite database (created on first run)
│   └── Master/                   # Excel master files
└── resources/
    └── icons/                    # Application icons
```

---

## 📦 Building an Executable

Create a standalone `.exe` with PyInstaller:

```bash
# Install build dependencies
pip install pyinstaller

# Build single-file executable
pyinstaller shiftpaste.spec --clean

# Output: dist/ShiftPaste.exe
```

### Creating an Installer

For a professional Windows installer:

1. Install [Inno Setup](https://jrsoftware.org/isdl.php)
2. Build the exe first: `pyinstaller shiftpaste.spec --clean`
3. Open `setup_script.iss` in Inno Setup Compiler
4. Click **Build → Compile**
5. Output: `ShiftPaste_Setup.exe`

---

## 🔧 Troubleshooting

| Issue | Solution |
|-------|----------|
| Hotkey doesn't work | Another app may use `Ctrl+Shift+V`. Change it in Settings. |
| Excel file not updating | Ensure file is `.xlsx` format and in `data/Master/` |
| `pathlib` error during build | Run `pip uninstall pathlib` (obsolete backport) |
| Missing DLLs error | Install [Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe) |
| High CPU usage | Increase search delay in Advanced settings |

---

## 🔒 Security Features

- **Password Manager Exclusion**: Content copied from KeePass, 1Password, Bitwarden, LastPass, Dashlane, and others is automatically excluded from history
- **No Network Access**: The app is fully offline - no data leaves your machine
- **User-Level Storage**: All data stored in user directory, no admin required
- **Content Hashing**: Deduplication uses SHA-256 hashing, not raw content comparison

---

## 📄 License

MIT License - See [LICENSE](LICENSE) for details.

---

## 👥 Credits

**Developed by:** [InfinityHardik](https://github.com/infinityhardik) + AI assistants (Gemini, Claude)

---

*Built for power users who need speed and precision.*
