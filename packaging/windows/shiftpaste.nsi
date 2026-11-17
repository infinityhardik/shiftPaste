; Shift Paste installer (NSIS 3).
; Build:  makensis /DVERSION=2.0.0 /DSRCDIR=..\..\target\release packaging\windows\shiftpaste.nsi
; - Asks for admin and installs for all users into Program Files.
; - Closes a running Shift Paste first (install and uninstall).
; - "Launch Shift Paste" starts it as the signed-in user, not as admin.
; - Uninstall asks whether to also delete the clipboard history, collections
;   and settings (unticked by default) for every account on the PC, so nothing
;   is left behind. Silent: uninstall.exe /S keeps data, /S /DELETEDATA doesn't.

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
!define DATA_PARENT "infinityhardik"          ; %LOCALAPPDATA% and %APPDATA%
!define DATA_DIR "${DATA_PARENT}\ShiftPaste"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define APPROVED_KEY "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"

Name "${APPNAME}"
OutFile "ShiftPaste-${VERSION}-Setup.exe"
RequestExecutionLevel admin
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\InfinityHardik\ShiftPaste" "InstallDir"
SetCompressor /SOLID lzma
ManifestDPIAware true
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
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "FileFunc.nsh"

Var DeleteData      ; 1 = uninstall also deletes history, collections and settings
Var DeleteDataBox
Var UnFinishText

!define MUI_ICON "${ASSETS}\shiftpaste.ico"
!define MUI_UNICON "${ASSETS}\shiftpaste.ico"
; Side picture of the welcome/finish pages (make-wizard-bitmap.sh).
!define MUI_WELCOMEFINISHPAGE_BITMAP "wizard.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "wizard.bmp"
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
UninstPage custom un.OptionsPage un.OptionsLeave
!insertmacro MUI_UNPAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "$UnFinishText"
!insertmacro MUI_UNPAGE_FINISH
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
  StrCpy $DeleteData 0
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/DELETEDATA" $R1
  ${IfNot} ${Errors}
    StrCpy $DeleteData 1
  ${EndIf}
FunctionEnd

; First uninstall page: what is removed, and whether to delete the data too.
Function un.OptionsPage
  !insertmacro MUI_HEADER_TEXT "Uninstall ${APPNAME}" "Remove ${APPNAME} from this computer."
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${NSD_CreateLabel} 0 0 100% 20u "${APPNAME} will be removed from this folder:$\r$\n$INSTDIR"
  Pop $0
  ${NSD_CreateCheckbox} 0 32u 100% 12u "&Also delete my clipboard history, collections and settings"
  Pop $DeleteDataBox
  ${If} $DeleteData == 1
    ${NSD_Check} $DeleteDataBox
  ${EndIf}
  ${NSD_CreateLabel} 12u 46u -12u 36u "Removes everything ${APPNAME} has saved, for every user account on this PC, so no ${APPNAME} files are left behind. This can't be undone.$\r$\n$\r$\nLeave it unticked to keep your data in case you reinstall."
  Pop $0
  GetDlgItem $0 $HWNDPARENT 1
  SendMessage $0 ${WM_SETTEXT} 0 "STR:$(^UninstallBtn)"
  nsDialogs::Show
FunctionEnd

Function un.OptionsLeave
  ${NSD_GetState} $DeleteDataBox $0
  ${If} $0 == ${BST_CHECKED}
    MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 \
      "Delete all ${APPNAME} clipboard history, collections and settings?$\r$\n$\r$\nThis can't be undone." \
      IDYES +2
    Abort
    StrCpy $DeleteData 1
  ${Else}
    StrCpy $DeleteData 0
  ${EndIf}
FunctionEnd

; Delete one profile's Shift Paste folder, and the parent if that leaves it empty.
!macro DeleteDataIn BASE
  RMDir /r "${BASE}\${DATA_DIR}"
  RMDir "${BASE}\${DATA_PARENT}"
!macroend

; Clipboard history, collections, settings and logs of every account.
Function un.DeleteAllUserData
  ; The account running the uninstaller (also finds redirected AppData folders).
  SetShellVarContext current
  !insertmacro DeleteDataIn "$LOCALAPPDATA"
  !insertmacro DeleteDataIn "$APPDATA"
  ; Every other account on this PC.
  StrCpy $R0 0
  ${Do}
    EnumRegKey $R1 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList" $R0
    ${If} $R1 == ""
      ${Break}
    ${EndIf}
    IntOp $R0 $R0 + 1
    ReadRegStr $R2 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$R1" "ProfileImagePath"
    ${If} $R2 != ""
      ExpandEnvStrings $R2 $R2
      !insertmacro DeleteDataIn "$R2\AppData\Local"
      !insertmacro DeleteDataIn "$R2\AppData\Roaming"
    ${EndIf}
  ${Loop}
  ; Portable-mode data next to the program, if someone created it.
  RMDir /r "$INSTDIR\ShiftPaste-data"
FunctionEnd

; Launch-at-login entries: the app writes them per user; older builds could
; also write the all-users ones. Covers every signed-in account.
Function un.RemoveStartupEntries
  DeleteRegValue HKCU "${RUN_KEY}" "${APPNAME}"
  DeleteRegValue HKCU "${APPROVED_KEY}" "${APPNAME}"
  DeleteRegValue HKLM "${RUN_KEY}" "${APPNAME}"
  DeleteRegValue HKLM "${APPROVED_KEY}" "${APPNAME}"
  StrCpy $R0 0
  ${Do}
    EnumRegKey $R1 HKU "" $R0
    ${If} $R1 == ""
      ${Break}
    ${EndIf}
    IntOp $R0 $R0 + 1
    DeleteRegValue HKU "$R1\${RUN_KEY}" "${APPNAME}"
    DeleteRegValue HKU "$R1\${APPROVED_KEY}" "${APPNAME}"
  ${Loop}
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
  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "Software\InfinityHardik\ShiftPaste"
  DeleteRegKey /ifempty HKLM "Software\InfinityHardik"
  Call un.RemoveStartupEntries
  ${If} $DeleteData == 1
    DetailPrint "Deleting clipboard history, collections and settings..."
    Call un.DeleteAllUserData
    StrCpy $UnFinishText "${APPNAME} and all its data have been removed from this computer."
  ${Else}
    StrCpy $UnFinishText "${APPNAME} has been removed from this computer.$\r$\n$\r$\nYour clipboard history, collections and settings were kept, in case you reinstall."
  ${EndIf}
  SetShellVarContext all
  RMDir "$INSTDIR"
SectionEnd
