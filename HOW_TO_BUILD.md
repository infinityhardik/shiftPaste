# How to create an EXE for Shift Paste

To create a standalone executable for Windows, follow these steps:

### 1. Install PyInstaller
Ensure you have the build tool installed:
```bash
pip install pyinstaller
```

### 2. Run the Build Command
Use the provided `.spec` file to package the application. This will handle all the icons, hidden imports, and internal structure correctly.

```bash
pyinstaller build.spec --clean
```

### 3. Locate your EXE
Once the process completes:
- You will find a single standalone file: `dist/ShiftPaste.exe`.
- You can distribute this file alone; it contains everything needed to run.

---

### Pro Tip: Creating a "Full Installer"
If you want a professional installer (the kind with a "Next > Next > Finish" wizard), I have provided a pre-configured **Inno Setup script**:

1. **Download**: [Inno Setup](https://jrsoftware.org/isdl.php)
2. **Icon**: I have generated a premium icon for you at `resources/icons/app_icon.png`. For the best results, convert this to an `.ico` file (using an online converter) and save it as `resources/icons/app_icon.ico`.
3. **Build**:
   - Open `setup_script.iss` in Inno Setup.
   - Click **Compile**.
   - It will generate a `ShiftPaste_Setup.exe` that handles installation, desktop shortcuts, and even auto-start configuration!

---
**Happy Packaging!**
