# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller build specification for Shift Paste.

Build command:
    pyinstaller shiftpaste.spec --clean

Output:
    dist/ShiftPaste.exe - Single standalone executable
"""

import os
import sys

# Suppress block_cipher deprecation - using default (None)
block_cipher = None

# Collect data files
datas = []

# Add resources if they exist
if os.path.exists('resources'):
    datas.append(('resources', 'resources'))

# CRITICAL: Add data directory for database and configs
# This is likely why clipboard reading fails - the app needs its data directory
if os.path.exists('data'):
    datas.append(('data', 'data'))

# Add any config files in root
for config_file in ['config.json', 'settings.json', 'config.ini']:
    if os.path.exists(config_file):
        datas.append((config_file, '.'))

# Add icons if they exist
icon_path = 'resources/icons/app_icon.ico'

# Analysis configuration
a = Analysis(
    ['main.py'],
    pathex=[os.getcwd()],
    binaries=[],
    datas=datas,
    hiddenimports=[
        # Qt modules
        'PySide6.QtCore',
        'PySide6.QtGui', 
        'PySide6.QtWidgets',
        # Data handling
        'openpyxl',
        'openpyxl.cell._reader',
        'openpyxl.cell._writer',
        # System utilities
        'psutil',
        # Windows-specific (clipboard related)
        'pynput.keyboard._win32',
        'pynput.mouse._win32',
        'win32gui',
        'win32con',
        'win32process',
        'win32clipboard',
        'win32api',
        'winreg',
        # Clipboard
        'pyperclip',
        # Additional Windows clipboard support
        'ctypes',
        'ctypes.wintypes',
        # Path utilities for frozen executable
        'src.utils.paths',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    # Exclude large unused packages
    excludes=[
        'pandas',
        'numpy', 
        'watchdog',
        'pyautogui',
        'keyboard',
        'matplotlib',
        'scipy',
        'PIL',
        'cv2',
        'tensorflow',
        'torch',
        'tkinter',
        '_tkinter',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

# Create PYZ archive
pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

# Executable configuration
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='ShiftPaste',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,  # Enable UPX compression for smaller size
    upx_exclude=[
        'vcruntime140.dll',
        'python*.dll',
        'Qt*.dll',
    ],
    runtime_tmpdir=None,
    console=False,  # No console window (GUI app)
    disable_windowed_traceback=False,
    target_arch=None,  # Auto-detect
    codesign_identity=None,
    entitlements_file=None,
    icon=icon_path if os.path.exists(icon_path) else None,
    version='version_info.txt' if os.path.exists('version_info.txt') else None,
)

# macOS bundle configuration (only used on macOS)
if sys.platform == 'darwin':
    app = BUNDLE(
        exe,
        name='ShiftPaste.app',
        icon='resources/icons/app_icon.icns',
        bundle_identifier='com.infinityhardik.shiftpaste',
        info_plist={
            'NSPrincipalClass': 'NSApplication',
            'NSHighResolutionCapable': True,
            'CFBundleShortVersionString': '1.1.0',
            'CFBundleVersion': '1.1.0',
            'LSMinimumSystemVersion': '10.13.0',
            'NSAppleEventsUsageDescription': 'Shift Paste needs to control other applications to paste content.',
        },
    )