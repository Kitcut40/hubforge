; =====================================================================
;  HubForge — installateur Windows (NSIS 3, Modern UI 2)
;
;  Installation par utilisateur, sans droits administrateur :
;    %LOCALAPPDATA%\Programs\HubForge
;  Les réglages et profils restent dans %APPDATA%\HubForge (jamais effacés
;  sans le demander).
;
;  Mode mise à jour (lancé par HubForge lui-même) :
;    HubForge-Setup-x.y.z.exe /S /UPDATE
;  → silencieux, ferme HubForge, installe, relance HubForge.
;
;  Compilation (voir installer/build.sh) :
;    makensis -DVERSION=0.8.0 -DSRC=<dossier des fichiers> -DOUT=<fichier.exe> hubforge.nsi
; =====================================================================

Target amd64-unicode  ; HubForge est un programme 64 bits
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!ifndef VERSION
  !error "VERSION manquant (-DVERSION=0.8.0)"
!endif
!ifndef SRC
  !error "SRC manquant (-DSRC=dossier contenant hubforge.exe)"
!endif
!ifndef OUT
  !define OUT "HubForge-Setup-${VERSION}.exe"
!endif

!define APP        "HubForge"
!define EXE        "hubforge.exe"
!define PUBLISHER  "HubForge (logiciel libre)"
!define URL        "https://github.com/Kitcut40/hubforge"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\HubForge"
!define RUN_KEY    "Software\Microsoft\Windows\CurrentVersion\Run"
!define WEBVIEW2   "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

Name "${APP}"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
InstallDirRegKey HKCU "Software\${APP}" "InstallDir"
BrandingText "${APP} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1036 "ProductName" "${APP}"
VIAddVersionKey /LANG=1036 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "FileDescription" "Installation de ${APP}"
VIAddVersionKey /LANG=1036 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=1036 "LegalCopyright" "GPL-3.0-or-later"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WinMessages.nsh"

; ---------------------------------------------------------------------
;  Apparence
; ---------------------------------------------------------------------
!define MUI_ICON   "../assets/brand/hubforge.ico"
!define MUI_UNICON "../assets/brand/hubforge.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "../assets/brand/installer-header.bmp"
!define MUI_HEADERIMAGE_UNBITMAP "../assets/brand/installer-header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "../assets/brand/installer-side.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "../assets/brand/installer-side.bmp"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_SMALLDESC

; ---------------------------------------------------------------------
;  Pages
; ---------------------------------------------------------------------
!define MUI_WELCOMEPAGE_TITLE "$(WelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(WelcomeText)"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "../LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "$(RunNow)"
!define MUI_FINISHPAGE_LINK "$(Website)"
!define MUI_FINISHPAGE_LINK_LOCATION "${URL}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "English"

; ---------------------------------------------------------------------
;  Textes (français / anglais)
; ---------------------------------------------------------------------
LangString WelcomeTitle ${LANG_FRENCH}  "Bienvenue dans l'installation de HubForge ${VERSION}"
LangString WelcomeTitle ${LANG_ENGLISH} "Welcome to HubForge ${VERSION} Setup"
LangString WelcomeText  ${LANG_FRENCH}  "HubForge est un logiciel libre pour le Cooler Master MasterHUB : touches, faders, molettes, LED, profils et addons.$\r$\n$\r$\nIl fonctionne entièrement hors ligne, sans compte.$\r$\n$\r$\nFerme le logiciel officiel MasterHUB s'il est ouvert : un seul logiciel peut piloter le MasterHUB à la fois.$\r$\n$\r$\nHubForge n'est pas affilié à Cooler Master."
LangString WelcomeText  ${LANG_ENGLISH} "HubForge is free software for the Cooler Master MasterHUB: keys, faders, rollers, LEDs, profiles and add-ons.$\r$\n$\r$\nIt works fully offline, no account needed.$\r$\n$\r$\nClose the official MasterHUB software if it is running: only one program can drive the MasterHUB at a time.$\r$\n$\r$\nHubForge is not affiliated with Cooler Master."
LangString RunNow       ${LANG_FRENCH}  "Lancer HubForge"
LangString RunNow       ${LANG_ENGLISH} "Launch HubForge"
LangString Website      ${LANG_FRENCH}  "HubForge sur GitHub (guide, addons, nouveautés)"
LangString Website      ${LANG_ENGLISH} "HubForge on GitHub (guide, add-ons, news)"
LangString SecMain      ${LANG_FRENCH}  "HubForge (obligatoire)"
LangString SecMain      ${LANG_ENGLISH} "HubForge (required)"
LangString SecMainDesc  ${LANG_FRENCH}  "Le logiciel, son raccourci dans le menu Démarrer et l'ouverture des addons .forge par double-clic."
LangString SecMainDesc  ${LANG_ENGLISH} "The program, its Start menu shortcut and opening .forge add-ons with a double-click."
LangString SecDesktop   ${LANG_FRENCH}  "Raccourci sur le Bureau"
LangString SecDesktop   ${LANG_ENGLISH} "Desktop shortcut"
LangString SecDesktopDesc ${LANG_FRENCH}  "Ajoute une icône HubForge sur le Bureau."
LangString SecDesktopDesc ${LANG_ENGLISH} "Adds a HubForge icon to the desktop."
LangString SecAuto      ${LANG_FRENCH}  "Démarrer avec Windows"
LangString SecAuto      ${LANG_ENGLISH} "Start with Windows"
LangString SecAutoDesc  ${LANG_FRENCH}  "HubForge se lance discrètement à l'ouverture de session (icône près de l'horloge), pour que le MasterHUB soit prêt tout de suite."
LangString SecAutoDesc  ${LANG_ENGLISH} "HubForge starts quietly when you sign in (icon near the clock), so the MasterHUB is ready right away."
LangString NoWebView    ${LANG_FRENCH}  "HubForge a besoin de Microsoft Edge WebView2, absent de ce PC (il est déjà présent sur presque tous les Windows 10 et 11).$\r$\n$\r$\nOuvrir la page de téléchargement de Microsoft ?"
LangString NoWebView    ${LANG_ENGLISH} "HubForge needs Microsoft Edge WebView2, which is missing on this PC (it ships with almost every Windows 10 and 11).$\r$\n$\r$\nOpen Microsoft's download page?"
LangString OfficialRunning ${LANG_FRENCH}  "Le logiciel officiel MasterHUB est ouvert. Ferme-le (icône près de l'horloge → Quitter) avant d'utiliser HubForge, sinon les deux logiciels se disputeront le MasterHUB."
LangString OfficialRunning ${LANG_ENGLISH} "The official MasterHUB software is running. Quit it (icon near the clock → Quit) before using HubForge, otherwise both programs will fight over the MasterHUB."
LangString KeepData     ${LANG_FRENCH}  "Supprimer aussi tes réglages, profils et addons ($APPDATA\HubForge) ?$\r$\n$\r$\nChoisis Non pour les garder (conseillé si tu réinstalles HubForge plus tard)."
LangString KeepData     ${LANG_ENGLISH} "Also delete your settings, profiles and add-ons ($APPDATA\HubForge)?$\r$\n$\r$\nChoose No to keep them (recommended if you reinstall HubForge later)."
LangString FileLocked   ${LANG_FRENCH}  "HubForge est encore ouvert. Quitte-le (icône près de l'horloge → Quitter HubForge) puis clique sur Réessayer."
LangString FileLocked   ${LANG_ENGLISH} "HubForge is still running. Quit it (icon near the clock → Quit HubForge) and click Retry."
LangString AddonFile    ${LANG_FRENCH}  "Addon HubForge"
LangString AddonFile    ${LANG_ENGLISH} "HubForge add-on"

Var IsUpdate

; ---------------------------------------------------------------------
;  Fermer HubForge s'il tourne (sinon ses fichiers sont verrouillés)
; ---------------------------------------------------------------------
!macro CloseHubForge
  ${If} ${FileExists} "$INSTDIR\${EXE}"
    ExecWait '"$INSTDIR\${EXE}" --quit'
  ${EndIf}
  Sleep 500
  nsExec::Exec 'taskkill /IM ${EXE} /T'
  Sleep 300
!macroend

Function .onInit
  StrCpy $IsUpdate "0"
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    StrCpy $IsUpdate "1"
  ${EndIf}

  ; Langue : français si Windows est en français, sinon anglais.
  System::Call 'kernel32::GetUserDefaultUILanguage() i .r0'
  IntOp $0 $0 & 0x3FF
  ${If} $0 == 0x0C
    StrCpy $LANGUAGE ${LANG_FRENCH}
  ${Else}
    StrCpy $LANGUAGE ${LANG_ENGLISH}
  ${EndIf}

  ; WebView2 (moteur de l'interface) présent ?
  ReadRegStr $1 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2}" "pv"
  ${If} $1 == ""
    ReadRegStr $1 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2}" "pv"
  ${EndIf}
  ${If} $1 == ""
    ReadRegStr $1 HKCU "Software\Microsoft\EdgeUpdate\Clients\${WEBVIEW2}" "pv"
  ${EndIf}
  ${If} $1 == ""
  ${OrIf} $1 == "0.0.0.0"
    ${IfNot} ${Silent}
      MessageBox MB_YESNO|MB_ICONEXCLAMATION "$(NoWebView)" IDNO +2
        ExecShell "open" "https://developer.microsoft.com/microsoft-edge/webview2/"
    ${EndIf}
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------
;  Sections
; ---------------------------------------------------------------------
Section "$(SecMain)" SecMain
  SectionIn RO
  !insertmacro CloseHubForge

  SetOutPath "$INSTDIR"
  ClearErrors
  retry_exe:
  File "${SRC}/${EXE}"
  ${If} ${Errors}
    ClearErrors
    ${If} ${Silent}
      Sleep 1500
      nsExec::Exec 'taskkill /F /IM ${EXE} /T'
      Sleep 800
      File "${SRC}/${EXE}"
    ${Else}
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(FileLocked)" IDRETRY retry_exe
      Abort
    ${EndIf}
  ${EndIf}
  File "${SRC}/WebView2Loader.dll"
  File "/oname=LICENSE.txt" "../LICENSE"
  File "/oname=ADDON-EXCEPTION.md" "../ADDON-EXCEPTION.md"
  File "/oname=CHANGELOG.md" "../CHANGELOG.md"

  ; Menu Démarrer
  CreateShortCut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0

  ; Addons : double-clic sur un fichier .forge
  WriteRegStr HKCU "Software\Classes\.forge" "" "HubForge.Addon"
  WriteRegStr HKCU "Software\Classes\HubForge.Addon" "" "$(AddonFile)"
  WriteRegStr HKCU "Software\Classes\HubForge.Addon\DefaultIcon" "" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "Software\Classes\HubForge.Addon\shell\open\command" "" '"$INSTDIR\${EXE}" --install-addon "%1"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  ; Désinstallation (Paramètres → Applications)
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "${URL}"
  WriteRegStr HKCU "${UNINST_KEY}" "HelpLink" "${URL}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"

  ; Le démarrage avec Windows pointe vers la version installée s'il était déjà activé.
  ReadRegStr $0 HKCU "${RUN_KEY}" "${APP}"
  ${If} $0 != ""
    WriteRegStr HKCU "${RUN_KEY}" "${APP}" '"$INSTDIR\${EXE}" --hidden'
  ${EndIf}

  ; Logiciel officiel ouvert ? On prévient (sans le fermer d'office).
  ${IfNot} ${Silent}
    nsExec::ExecToStack 'tasklist /FI "IMAGENAME eq MasterHUB.exe" /NH'
    Pop $0
    Pop $1
    Push $1
    Push "MasterHUB.exe"
    Call StrContains
    Pop $3
    ${If} $3 != ""
      MessageBox MB_OK|MB_ICONINFORMATION "$(OfficialRunning)"
    ${EndIf}
  ${EndIf}
SectionEnd

Section "$(SecDesktop)" SecDesktop
  ${If} $IsUpdate == "0"
    CreateShortCut "$DESKTOP\${APP}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0
  ${EndIf}
SectionEnd

Section "$(SecAuto)" SecAuto
  ${If} $IsUpdate == "0"
    WriteRegStr HKCU "${RUN_KEY}" "${APP}" '"$INSTDIR\${EXE}" --hidden'
  ${EndIf}
SectionEnd

; Mise à jour silencieuse : relancer HubForge à la fin.
Section "-Relance"
  ${If} $IsUpdate == "1"
    Exec '"$INSTDIR\${EXE}"'
  ${EndIf}
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "$(SecMainDesc)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "$(SecDesktopDesc)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAuto} "$(SecAutoDesc)"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; Recherche d'un texte dans un autre : Push texte, Push motif, Call → Pop (motif ou "")
Function StrContains
  Exch $R1 ; motif
  Exch
  Exch $R2 ; texte
  Push $R3
  Push $R4
  Push $R5
  StrLen $R3 $R1
  StrCpy $R4 0
  loop:
    StrCpy $R5 $R2 $R3 $R4
    StrCmp $R5 $R1 found
    StrCmp $R5 "" notfound
    IntOp $R4 $R4 + 1
    Goto loop
  found:
    StrCpy $R1 $R1
    Goto done
  notfound:
    StrCpy $R1 ""
  done:
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Exch $R1
FunctionEnd

; ---------------------------------------------------------------------
;  Désinstallation
; ---------------------------------------------------------------------
Function un.onInit
  System::Call 'kernel32::GetUserDefaultUILanguage() i .r0'
  IntOp $0 $0 & 0x3FF
  ${If} $0 == 0x0C
    StrCpy $LANGUAGE ${LANG_FRENCH}
  ${Else}
    StrCpy $LANGUAGE ${LANG_ENGLISH}
  ${EndIf}
FunctionEnd

Section "Uninstall"
  ${If} ${FileExists} "$INSTDIR\${EXE}"
    ExecWait '"$INSTDIR\${EXE}" --quit'
  ${EndIf}
  Sleep 500
  nsExec::Exec 'taskkill /F /IM ${EXE} /T'
  Sleep 300

  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\ADDON-EXCEPTION.md"
  Delete "$INSTDIR\CHANGELOG.md"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP}.lnk"
  Delete "$DESKTOP\${APP}.lnk"

  ; Démarrage avec Windows : seulement s'il pointe vers cette installation.
  ReadRegStr $0 HKCU "${RUN_KEY}" "${APP}"
  ${If} $0 == '"$INSTDIR\${EXE}" --hidden'
    DeleteRegValue HKCU "${RUN_KEY}" "${APP}"
  ${EndIf}

  ReadRegStr $0 HKCU "Software\Classes\.forge" ""
  ${If} $0 == "HubForge.Addon"
    DeleteRegKey HKCU "Software\Classes\.forge"
  ${EndIf}
  DeleteRegKey HKCU "Software\Classes\HubForge.Addon"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\${APP}"

  ; Réglages et profils : seulement si l'utilisateur le demande (jamais en silencieux).
  ${IfNot} ${Silent}
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "$(KeepData)" IDNO keep
      RMDir /r "$APPDATA\HubForge"
    keep:
  ${EndIf}
SectionEnd
