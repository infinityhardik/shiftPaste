# SHIFT PASTE - QUICK REFERENCE CARD

## 🚀 Quick Start (2 Minutes)

```bash
# 1. Install dependencies
pip install -r requirements.txt

# 2. Run the app
python main.py

# 3. Open Shift Paste
Ctrl+Shift+V (Windows)
Shift+Cmd+V (macOS)  
Shift+Super+V (Linux)

# 4. Type to search, press Enter to paste
```

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+V` | Toggle Shift Paste (Windows) |
| `Shift+Cmd+V` | Toggle Shift Paste (macOS) |
| `Shift+Super+V` | Toggle Shift Paste (Linux) |
| `Enter` | Paste selected item |
| `Delete` | Delete clipboard item |
| `Esc` | Close window |
| `↑/↓` | Navigate items |

---

## 🔍 Search Examples

**Works**:
- `mrlx` → "MARLEX A Grade 100%"
- `fd` → "Flush Door"
- `grade 100` → "A Grade 100%"

**Doesn't Work**:
- `xml` → "XML config" (not left-to-right)
- `door mar` → "MARLEX Door" (order reversed)

---

## 📁 Master Files Location

```
data/Master/
├── Pinned.xlsx     (Important items)
├── Work.xlsx       (Work snippets)
└── Personal.xlsx   (Personal items)
```

Edit directly in Excel - changes auto-reload!

---

## ⚙️ Configuration

**File**: `config.json`

```json
{
  "clipboard": {
    "max_items": 20,        # Items to store
    "preview_chars": 100    # Preview length
  },
  "shortcuts": {
    "windows": "ctrl+shift+v"
  },
  "master_file": {
    "auto_reload": true
  },
  "ui": {
    "theme": "system"
  }
}
```

---

## 📊 Performance

| Metric | Value |
|--------|-------|
| Window Opens | <100ms |
| Search Results | <50ms |
| Memory Usage | ~30MB idle |
| Startup Time | ~2 seconds |

---

## 🐛 Troubleshooting

### Hotkey not working
→ Change in Settings, check for conflicts

### Items not syncing
→ Ensure Excel files in `data/Master/`, has headers

### Paste not working
→ Click in target window first, or manually paste

### Database error
→ Delete `data/clipboard.db`, restart app

---

## 📚 File Structure

```
shift-paste/
├── main.py              # Run this
├── config.json          # Settings
├── data/
│   ├── clipboard.db     # Database
│   └── Master/          # Excel files
├── src/
│   ├── app.py           # Main controller
│   ├── ui/              # User interface
│   ├── core/            # Search & monitoring
│   └── data/            # Database & files
└── README.md            # Full guide
```

---

## 🔨 Building Executable

```bash
# Install PyInstaller
pip install pyinstaller

# Build
pyinstaller build.spec --clean

# Run executable
# Windows: dist/ShiftPaste.exe
# macOS: dist/ShiftPaste.app
# Linux: dist/ShiftPaste
```

---

## 🧪 Running Tests

```bash
# Test search engine
python tests/test_search.py

# Test database
python tests/test_database.py
```

---

## 💾 Database Details

| Table | Purpose |
|-------|---------|
| clipboard_items | Recent clipboard history |
| master_items | Persistent master items |
| search_index | FTS5 full-text search |

---

## 🎯 Key Features

✨ **Automatic** - Clipboard monitored 24/7
🔍 **Smart** - Fuzzy search with ranking
📁 **Organized** - Excel-based collections
⚡ **Fast** - <100ms response time
🔄 **Synced** - Auto-reload Excel changes
💾 **Persistent** - Everything saved locally
🌍 **Cross-Platform** - Windows, macOS, Linux

---

## 📖 Full Documentation

- **Setup Guide**: SETUP.md
- **User Guide**: README.md
- **Implementation Details**: IMPLEMENTATION_SUMMARY.md

---

## 🚨 Common Issues

| Issue | Solution |
|-------|----------|
| Hotkey conflict | Change in Settings |
| Excel not syncing | Reload in Settings |
| Paste fails | Click target window |
| High memory | Reduce max_items |
| DB locked | Close app, delete .db |

---

## 💡 Pro Tips

1. Use `Pinned.xlsx` for frequently used items
2. Organize snippets by category
3. Search is left-to-right: `mrlx` finds MARLEX
4. Master items always ranked higher
5. Delete removes clipboard items only
6. Master items permanent (delete from Excel)

---

## 🔗 Quick Links

- Main: `python main.py`
- Settings: Right-click tray → "Settings"
- Master Files: Edit `data/Master/*.xlsx`
- Logs: Check terminal output
- Rebuild: `python tests/test_database.py`

---

**v1.0.0 - Built for Productivity** ✨

Questions? See README.md or SETUP.md for details.
