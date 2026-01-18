# SHIFT PASTE - DEVELOPER DOCUMENTATION

## Overview

Shift Paste is a complete clipboard manager with fuzzy search, Excel integration, and cross-platform support. This document provides technical details for developers.

---

## Architecture

### Layer Model

```
┌─────────────────────────────────────┐
│      User Interface Layer           │
│   (PySide6 Qt6 Widgets)            │
├─────────────────────────────────────┤
│   Application Controller Layer      │
│   (src/app.py)                     │
├─────────────────────────────────────┤
│    Business Logic Layer             │
│   ├─ Search Engine                 │
│   ├─ Clipboard Monitor             │
│   └─ Hotkey Manager                │
├─────────────────────────────────────┤
│     Data Access Layer               │
│   ├─ Database (SQLite)             │
│   ├─ Excel Manager                 │
│   └─ Configuration                 │
├─────────────────────────────────────┤
│      External Services              │
│   ├─ pyperclip (Clipboard)         │
│   ├─ watchdog (File Monitoring)    │
│   ├─ keyboard (Global Hotkeys)     │
│   └─ pyautogui (Paste Simulation)  │
└─────────────────────────────────────┘
```

---

## Module Reference

### `src/app.py` - Main Application Controller

**Purpose**: Orchestrates all components, manages lifecycle

**Key Classes**:
- `ShiftPasteApp` - Main application class

**Key Methods**:
- `__init__()` - Initialize all components
- `_setup_ui()` - Initialize user interface
- `_setup_services()` - Start background services
- `_setup_hotkeys()` - Register global hotkeys
- `show_main_window()` - Display clipboard manager
- `toggle_main_window()` - Toggle visibility
- `_on_search_changed()` - Handle search queries
- `_on_item_selected()` - Handle item selection
- `run()` - Start application event loop

**Event Flow**:
```
Startup
  ├─ Load config.json
  ├─ Initialize database
  ├─ Load master files
  ├─ Start clipboard monitor
  ├─ Register global hotkeys
  └─ Show system tray icon

User Action (Hotkey pressed)
  ├─ Load recent items
  ├─ Load master items
  ├─ Display main window
  └─ Focus search input

Search
  ├─ Query FTS5 database
  ├─ Apply fuzzy ranking
  ├─ Sort by score
  └─ Update UI

Selection
  ├─ Copy to clipboard
  ├─ Simulate Ctrl+V
  ├─ Close window
  └─ Cleanup

Shutdown
  ├─ Stop clipboard monitor
  ├─ Close database
  └─ Exit application
```

---

### `src/core/search_engine.py` - Fuzzy Search

**Purpose**: Implements fuzzy search algorithm with ranking

**Key Classes**:
- `SearchMatch` - NamedTuple for results
- `FuzzySearchEngine` - Search implementation

**Key Methods**:
- `search(query, items, max_results)` - Main search function
- `_calculate_match_quality()` - Score match quality
- `_find_leftright_matches()` - Find character positions
- `_score_positions()` - Score based on positions
- `_calculate_recency()` - Calculate time-based score
- `get_time_ago_string()` - Format timestamp

**Algorithm**:

```python
# 1. Left-to-right matching
"mrlx" in "MARLEX A Grade"
M -> found at 0
A -> found at 1
R -> found at 2
L -> found at 3
E -> found at 4
X -> found at 5

# 2. Calculate quality score
- Consecutive chars: +5.0
- Word boundary: +4.0 / +3.0
- Base match: +2.0
- Gap penalty: -0.5 per gap

# 3. Calculate recency score
age_hours = (now - timestamp) / 3600
recency = 1.0 / (1.0 + age_hours / 168)

# 4. Final score
score = (quality * 0.6 + recency * 0.4) * master_boost
```

**Ranking Strategy**:
1. Match quality (60%) - How well does text match
2. Recency (40%) - How recent is the item
3. Master boost (1.1x) - Master items prioritized

---

### `src/core/clipboard_monitor.py` - Background Monitoring

**Purpose**: Monitors clipboard for changes

**Key Classes**:
- `ClipboardMonitor` - QThread subclass

**Signals**:
- `clipboard_changed` - Emits new clipboard content

**Behavior**:
```
Every 200ms:
  1. Call pyperclip.paste()
  2. Check if content changed
  3. If changed, emit signal
  4. Update last_content
```

**Thread Safety**:
- Runs in separate QThread
- Uses signals for thread-safe communication
- Emits to main thread slot

---

### `src/data/database.py` - SQLite Operations

**Purpose**: Manage SQLite database with FTS5 search

**Key Classes**:
- `Database` - Database handler

**Tables**:
```sql
clipboard_items (id, content, timestamp, source_app)
master_items (id, content, category, timestamp, notes, is_active)
search_index (content, source_table, source_id) [FTS5]
```

**Key Methods**:
- `add_clipboard_item()` - Add clipboard entry
- `get_recent_items()` - Retrieve recent items
- `get_all_master_items()` - Retrieve master items
- `search_all_items()` - FTS5 search
- `delete_clipboard_item()` - Delete item
- `add_master_item()` - Add master item
- `sync_master_from_excel()` - Update from Excel

**FTS5 Search**:
```python
# Convert query to FTS5 format
"python" -> '("python")'
"python excel" -> '("python") OR ("excel")'

# Query with MATCH
SELECT * FROM search_index WHERE search_index MATCH ?
```

**Optimization**:
- Automatic indexing on insert
- Indexes on timestamp and category
- Full-text search for speed
- Duplicate detection

---

### `src/data/excel_manager.py` - Excel Handling

**Purpose**: Import/export and monitor Excel files

**Key Classes**:
- `ExcelFileHandler` - Watchdog event handler
- `ExcelWatcher` - QThread for monitoring
- `ExcelManager` - High-level manager

**Signals** (from watcher):
- `file_changed` - Excel file modified

**Key Methods**:
- `import_from_excel()` - Read items from Excel
- `export_to_excel()` - Write items to Excel
- `get_all_categories()` - List available categories
- `start_watching()` - Begin monitoring
- `stop_watching()` - Stop monitoring

**Excel Format**:
```
┌─────────────────────────────────────────────┐
│ Content | Timestamp | Notes                 │
├─────────────────────────────────────────────┤
│ Your    │ 2026-01-18 | Optional             │
│ text    │ 10:30:00   | notes                │
└─────────────────────────────────────────────┘
```

**Auto-reload Behavior**:
1. Watchdog detects file modification
2. Wait 500ms (ensure write complete)
3. Re-import from Excel
4. Clear old items for that category
5. Insert new items
6. Rebuild search index

---

### `src/data/config_manager.py` - Configuration

**Purpose**: Manage JSON configuration with persistence

**Key Classes**:
- `ConfigManager` - Configuration handler

**Default Config**:
```json
{
  "clipboard": {"max_items": 20, "preview_chars": 100},
  "shortcuts": {"windows": "ctrl+shift+v", ...},
  "master_file": {"directory": "data/Master", "auto_reload": true},
  "ui": {"theme": "system", "max_visible_items": 8},
  "startup": {"run_on_boot": false}
}
```

**Key Methods**:
- `load()` - Load from JSON file
- `save()` - Save to JSON file
- `get(key_path, default)` - Get value by dot notation
- `set(key_path, value)` - Set value by dot notation
- `get_all()` - Get entire config

**Dot Notation**:
```python
config.get('clipboard.max_items')  # Returns 20
config.set('ui.theme', 'dark')     # Sets theme to dark
```

---

### `src/ui/main_window.py` - Main UI Window

**Purpose**: Display clipboard manager popup

**Key Classes**:
- `ClipboardItemWidget` - Individual item display
- `MainWindow` - Main popup window

**Signals**:
- `item_selected` - User selected item
- `search_changed` - Search query changed
- `item_deleted` - Item deleted
- `window_closed` - Window closed

**Key Methods**:
- `show_near_cursor()` - Show at cursor position
- `update_results()` - Update displayed items
- `keyPressEvent()` - Handle keyboard input

**Layout**:
```
┌─────────────────────────────┐
│ Search Input                 │
├─────────────────────────────┤
│ Item 1 (preview + metadata)  │
├─────────────────────────────┤
│ Item 2 (preview + metadata)  │
├─────────────────────────────┤
│ Item 3 (preview + metadata)  │
├─────────────────────────────┤
│ Footer: Keyboard shortcuts   │
└─────────────────────────────┘
```

**Keyboard Handling**:
- Enter → Paste selected item
- Delete → Remove clipboard item
- Esc → Close window
- Click outside → Close window

---

### `src/ui/settings_window.py` - Settings Dialog

**Purpose**: Allow user configuration

**Key Classes**:
- `SettingsWindow` - Settings dialog

**Signals**:
- `settings_changed` - Settings updated

**Sections**:
1. Clipboard (max items, preview length)
2. Shortcuts (platform-specific hotkeys)
3. Master Files (auto-reload)
4. UI (theme, visible items)
5. Startup (run on boot)

---

### `src/ui/styles.py` - UI Styling

**Purpose**: QSS stylesheets for consistent look

**Functions**:
- `get_stylesheet()` - Light theme (Windows 10 style)
- `get_dark_stylesheet()` - Dark theme

**Style Elements**:
- Main window
- Search input
- Results list
- Scrollbar
- Buttons
- Menus

---

## Data Flow Diagrams

### Clipboard Monitoring Flow

```
System Clipboard
       ↓
[ClipboardMonitor Thread]
   (200ms poll)
       ↓
pyperclip.paste()
       ↓
Changed? (compare with last)
  YES ↓ NO
      └─ (do nothing)
      
      emit clipboard_changed(content)
       ↓
[ShiftPasteApp._on_clipboard_changed()]
       ↓
Database.add_clipboard_item(content)
       ↓
SQLite INSERT + FTS5 index
       ↓
Auto-trim if over max_items
```

### Search Flow

```
User types in search box
       ↓
[_on_search_changed(query)]
       ↓
Database.search_all_items(query)
       ↓
FTS5 full-text search
       ↓
Convert to list of items
       ↓
FuzzySearchEngine.search(query, items)
       ↓
For each item:
  - Calculate match_quality
  - Calculate recency
  - Apply master boost
  - Combine scores (60/40)
       ↓
Sort by final_score DESC
       ↓
Return top 20 results
       ↓
MainWindow.update_results()
       ↓
Render in UI
```

### Paste Flow

```
User presses Enter or double-clicks
       ↓
[_on_item_selected(item)]
       ↓
Close MainWindow
       ↓
pyperclip.copy(item.content)
       ↓
pyautogui.hotkey('ctrl', 'v')
       ↓
Content pasted to active window
```

---

## Configuration Options Reference

### Clipboard Settings
- `max_items` (10-500): Number of clipboard items to store
- `preview_chars` (50-200): Characters shown in preview

### Shortcut Settings
- `windows`: Hotkey for Windows (default: "ctrl+shift+v")
- `macos`: Hotkey for macOS (default: "shift+cmd+v")
- `linux`: Hotkey for Linux (default: "shift+super+v")

### Master File Settings
- `directory`: Path to Excel files (default: "data/Master")
- `auto_reload`: Auto-reload on external changes (default: true)

### UI Settings
- `theme`: "system", "light", or "dark" (default: "system")
- `max_visible_items`: Items shown at once (5-20, default: 8)

### Startup Settings
- `run_on_boot`: Launch on system startup (default: false)

---

## Database Schema

### clipboard_items Table

```sql
CREATE TABLE clipboard_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    source_app TEXT
);

CREATE INDEX idx_clipboard_timestamp 
ON clipboard_items(timestamp DESC);
```

**Queries**:
```sql
-- Get recent items
SELECT * FROM clipboard_items 
ORDER BY timestamp DESC LIMIT 20;

-- Check for duplicates
SELECT content FROM clipboard_items 
ORDER BY timestamp DESC LIMIT 1;

-- Delete old items
DELETE FROM clipboard_items WHERE id NOT IN (
  SELECT id FROM clipboard_items 
  ORDER BY timestamp DESC LIMIT 20
);
```

### master_items Table

```sql
CREATE TABLE master_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    category TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    notes TEXT,
    is_active BOOLEAN DEFAULT 1
);

CREATE INDEX idx_master_category 
ON master_items(category, timestamp DESC);
```

### search_index Table (FTS5)

```sql
CREATE VIRTUAL TABLE search_index 
USING fts5(
    content,
    source_table,
    source_id,
    tokenize = 'porter unicode61'
);
```

**Porter Tokenizer**: Removes common suffixes (e.g., "running" → "run")

---

## Performance Optimization

### Database Optimization
1. **FTS5 Full-Text Search**: ~1000x faster than LIKE queries
2. **Automatic Indexing**: Indexes on timestamp, category
3. **Batch Operations**: Sync from Excel in bulk
4. **Query Limits**: Always limit results (max 50)

### Search Optimization
1. **Position-based Scoring**: Avoid string operations
2. **Early Exit**: Stop searching after finding match
3. **Caching**: Score calculations cached in results
4. **Result Limit**: Cap at 20 items shown

### UI Optimization
1. **Lazy Loading**: Items rendered on demand
2. **Efficient Updates**: Partial list updates only
3. **Thread Isolation**: Long operations in background
4. **Event Debouncing**: Reduce search frequency

---

## Testing Guide

### Unit Tests

**test_search.py**:
```python
# Test fuzzy matching
items = [{"content": "MARLEX A Grade 100%", ...}]
results = engine.search("mrlx", items)
assert len(results) > 0
assert results[0].score > 0.8

# Test time formatting
assert "Just now" in engine.get_time_ago_string(now_timestamp)
```

**test_database.py**:
```python
# Test CRUD operations
db = Database(":memory:")  # In-memory for tests
id = db.add_clipboard_item("Test content")
assert id > 0

# Test search
results = db.search_all_items("test")
assert len(results) > 0
```

### Integration Tests

Run full application:
```bash
python main.py
# Test hotkey
# Test search
# Test paste
# Test settings
```

### Performance Tests

Monitor:
- CPU usage (should be <5% idle)
- Memory usage (should be <50MB)
- Search latency (should be <50ms)
- Window open time (should be <100ms)

---

## Debugging

### Enable Debug Logging

Add to `src/app.py`:
```python
import logging
logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger(__name__)
logger.debug("Message here")
```

### Common Issues

**Clipboard not monitoring**:
- Check `clipboard_monitor` thread is running
- Verify pyperclip works: `python -c "import pyperclip; print(pyperclip.paste())"`

**Search not returning results**:
- Verify database has items
- Check FTS5 query syntax
- Use `db.search_all_items("")` to debug

**Hotkey not working**:
- Check keyboard library installation
- Verify hotkey format (e.g., "ctrl+shift+v")
- Check for conflicts with other apps

**Excel not syncing**:
- Check file location: `data/Master/`
- Verify Excel format (headers: Content, Timestamp, Notes)
- Check file permissions

---

## Extension Points

### Add New Search Feature

In `src/core/search_engine.py`:
```python
def search_with_regex(self, pattern, items):
    """Add regex search"""
    import re
    results = []
    for item in items:
        if re.search(pattern, item['content']):
            results.append(item)
    return results
```

### Add New Master File Category

Auto-created by placing Excel file in `data/Master/`:
```
data/Master/CustomCategory.xlsx
```

### Add Custom Setting

In `src/data/config_manager.py` DEFAULT_CONFIG:
```json
"custom": {
  "my_setting": "value"
}
```

Access via:
```python
config.get('custom.my_setting')
```

---

## Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| PySide6 | 6.6.1 | Qt6 GUI |
| pyperclip | 1.8.2 | Clipboard access |
| keyboard | 0.13.5 | Global hotkeys |
| openpyxl | 3.1.2 | Excel read/write |
| pandas | 2.1.4 | Data processing |
| watchdog | 3.0.0 | File monitoring |
| pyautogui | 0.9.53 | Paste automation |
| pyinstaller | 6.3.0 | Executable building |

---

## Building for Production

### Steps

1. **Install PyInstaller**:
   ```bash
   pip install pyinstaller
   ```

2. **Build executable**:
   ```bash
   pyinstaller build.spec --clean
   ```

3. **Verify output**:
   ```bash
   # Windows
   dist/ShiftPaste.exe --version
   
   # macOS
   dist/ShiftPaste.app/Contents/MacOS/ShiftPaste --version
   
   # Linux
   dist/ShiftPaste --version
   ```

4. **Test thoroughly**:
   - All features work
   - Settings persist
   - No error messages
   - Startup time acceptable

5. **Distribute**:
   - Create release on GitHub
   - Upload executable
   - Document system requirements

---

## Version History

- **v1.0.0** (Jan 2026) - Initial release
  - All core features implemented
  - Cross-platform support
  - Complete documentation

---

## Support

For issues:
1. Check logs in terminal
2. Review this documentation
3. Check test files for examples
4. Review error messages carefully

For contributions:
1. Follow code style (PEP 8)
2. Add docstrings
3. Include tests
4. Update documentation

---

**Shift Paste Developer Documentation**

Built for extensibility and maintainability ✨
