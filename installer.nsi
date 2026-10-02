; ScreenBuddy NSIS Installer
; Build with: makensis installer.nsi

!include "MUI2.nsh"

Name "ScreenBuddy"
OutFile "target/screenbuddy-setup.exe"
InstallDir "$PROGRAMFILES\ScreenBuddy"
RequestExecutionLevel admin

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

Section "Install"
    SetOutPath "$INSTDIR"
    File "target\release\screenbuddy.exe"
    File /r "assets"
    CreateDirectory "$SMPROGRAMS\ScreenBuddy"
    CreateShortcut "$SMPROGRAMS\ScreenBuddy\ScreenBuddy.lnk" "$INSTDIR\screenbuddy.exe"
    CreateShortcut "$SMPROGRAMS\ScreenBuddy\Uninstall.lnk" "$INSTDIR\uninstall.exe"
    WriteUninstaller "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ScreenBuddy" "DisplayName" "ScreenBuddy"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ScreenBuddy" "UninstallString" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\screenbuddy.exe"
    Delete "$INSTDIR\uninstall.exe"
    RMDir /r "$INSTDIR\assets"
    Delete "$SMPROGRAMS\ScreenBuddy\ScreenBuddy.lnk"
    Delete "$SMPROGRAMS\ScreenBuddy\Uninstall.lnk"
    RMDir "$SMPROGRAMS\ScreenBuddy"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\ScreenBuddy"
SectionEnd
