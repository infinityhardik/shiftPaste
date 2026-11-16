; Shift Paste installer (NSIS 3).
; Build:  makensis /DVERSION=2.0.0 /DSRCDIR=..\..\target\release packaging\windows\shiftpaste.nsi
; - Asks for admin and installs for all users into Program Files.
; - Closes a running Shift Paste first (install and uninstall).
; - "Launch Shift Paste" starts it as the signed-in user, not as admin.
; - Uninstall can also delete the user's data (unticked by default).

Unicode true
!ifndef VERSION
  !define VERSION "2.0.0"
!endif
!ifndef SRCDIR
  !define SRCDIR "..\..\target\release"
!endif
!define APPNAME "Shift Paste"
!define EXE "shiftpaste.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ShiftPaste"
!define ASSETS "..\..\crates\app\assets"

Name "${APPNAME}"
OutFile "ShiftPaste-${VERSION}-Setup.exe"
RequestExecutionLevel admin
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\InfinityHardik\ShiftPaste" "InstallDir"
SetCompressor /SOLID lzma
BrandingText "${APPNAME} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileDescription" "${APPNAME} installer"
VIAddVersionKey "CompanyName" "InfinityHardik"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT License"

!include "MUI2.nsh"
!include "x64.nsh"

!define MUI_ICON "${ASSETS}\shiftpaste.ico"
!define MUI_UNICON "${ASSETS}\shiftpaste.ico"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_NODESC
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Launch ${APPNAME}"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchAsUser

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_COMPONENTS
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

; Ask a running copy to quit, then make sure it's gone (it may lock the exe).
!macro CloseShiftPaste
  IfFileExists "$INSTDIR\${EXE}" 0 +3
    nsExec::Exec '"$INSTDIR\${EXE}" --quit'
    Sleep 1000
  nsExec::Exec 'taskkill /IM ${EXE} /F'
  Sleep 300
!macroend

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${APPNAME} needs 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

; explorer.exe starts the app with the signed-in user's normal rights.
Function LaunchAsUser
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\${EXE}"'
FunctionEnd

Section "${APPNAME} (required)" SecApp
  SectionIn RO
  !insertmacro CloseShiftPaste
  SetShellVarContext all
  SetOutPath "$INSTDIR"
  File "${SRCDIR}\${EXE}"
  File /oname=LICENSE.txt "..\..\LICENSE"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0

  WriteRegStr HKLM "Software\InfinityHardik\ShiftPaste" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "InfinityHardik"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "https://github.com/infinityhardik/shiftPaste"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section /o "Desktop shortcut" SecDesktop
  SetShellVarContext all
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0
SectionEnd

Section "Uninstall"
  !insertmacro CloseShiftPaste
  SetShellVarContext all
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "Software\InfinityHardik\ShiftPaste"
  ; Launch-at-login entry written by the app (per user).
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${APPNAME}"
SectionEnd

Section /o "un.Also delete my data (history, collections, settings)" SecUnData
  SetShellVarContext current
  RMDir /r "$LOCALAPPDATA\infinityhardik\ShiftPaste"
  RMDir /r "$APPDATA\infinityhardik\ShiftPaste"
SectionEnd
