# Building Shift Paste

This guide explains how to build Shift Paste into a standalone Windows executable.

---

## Prerequisites

1. **Python 3.10+** installed
2. **Dependencies installed**: `pip install -r requirements.txt`
3. **PyInstaller**: `pip install pyinstaller`

---

## Quick Build

```bash
# Build single-file executable
pyinstaller shiftpaste.spec --clean
```

**Output:** `dist/ShiftPaste.exe`

This single file contains everything needed to run the application.

---

## Build Options

### Debug Build (with console)

For troubleshooting, create a build with visible console output:

```bash
# Edit shiftpaste.spec and change:
#   console=False  →  console=True
# Then rebuild:
pyinstaller shiftpaste.spec --clean
```

### Smaller Build (without UPX)

If UPX causes antivirus issues:

```bash
# Edit shiftpaste.spec and change:
#   upx=True  →  upx=False
# Then rebuild:
pyinstaller shiftpaste.spec --clean
```

---

## Creating a Windows Installer

For a professional "Next → Next → Finish" installer:

### 1. Download Inno Setup

Get it from: https://jrsoftware.org/isdl.php

### 2. Build the EXE First

```bash
pyinstaller shiftpaste.spec --clean
```

### 3. Compile the Installer

1. Open `setup_script.iss` in Inno Setup Compiler
2. Click **Build → Compile** (or press F9)
3. Output: `ShiftPaste_Setup.exe`

The installer includes:
- Desktop shortcut (optional)
- Start menu entry
- Auto-start option
- Uninstaller
- **Automatic app closing** - If Shift Paste is running during install/uninstall, you'll be prompted to close it automatically (prevents "file in use" errors)

---

## Troubleshooting Build Issues

### "pathlib is an obsolete backport"

```bash
pip uninstall pathlib
pyinstaller shiftpaste.spec --clean
```

### "__file__ is not defined"

The spec file uses `os.getcwd()` instead of `__file__`. If you see this error, ensure you're running from the project root.

### "DLL not found" at runtime

Install the Visual C++ Redistributable:
https://aka.ms/vs/17/release/vc_redist.x64.exe

### Build takes too long

PyInstaller analyzes all imports. First build is slow, subsequent builds are faster due to caching.

### Antivirus flags the exe

Some antivirus software flags PyInstaller executables. Options:
1. Disable UPX compression (`upx=False` in spec)
2. Sign the executable with a code signing certificate
3. Submit false positive report to your antivirus vendor

---

## Distribution Checklist

Before distributing:

- [ ] Test on a clean Windows machine (or VM)
- [ ] Verify hotkey works
- [ ] Verify system tray icon appears
- [ ] Test paste functionality in multiple apps
- [ ] Test settings persistence
- [ ] Verify no console window appears

---

## Deployment Locations

The application uses these data directories:

| Path | Purpose |
|------|---------|
| `data/clipboard.db` | SQLite database |
| `data/Master/` | Excel master files |
| `resources/icons/` | Application icons |

When installed via Inno Setup, these are created in the installation directory.

---

## Version Information

To embed version info in the exe, create `version_info.txt`:

```text
VSVersionInfo(
  ffi=FixedFileInfo(
    filevers=(1, 1, 0, 0),
    prodvers=(1, 1, 0, 0),
    OS=0x40004,
    fileType=0x1,
  ),
  kids=[
    StringFileInfo([
      StringTable(
        u'040904B0',
        [StringStruct(u'CompanyName', u'InfinityHardik'),
         StringStruct(u'FileDescription', u'Shift Paste Clipboard Manager'),
         StringStruct(u'FileVersion', u'1.1.0'),
         StringStruct(u'ProductName', u'Shift Paste'),
         StringStruct(u'ProductVersion', u'1.1.0')])
    ]),
    VarFileInfo([VarStruct(u'Translation', [1033, 1200])])
  ]
)
```

Then rebuild - PyInstaller will automatically include it.

---

**Happy Packaging! 📦**
