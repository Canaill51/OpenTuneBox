; Installateur Windows d'OpenTuneBox (Inno Setup 6), à partir du dossier dist\OpenTuneBox de PyInstaller.
;   iscc /DAppVersion=0.2.0 packaging\windows\opentunebox.iss   →   dist\OpenTuneBox-Windows-Setup.exe
; Installation pour l'utilisateur seul par défaut (pas de droits administrateur), ou pour tous au choix.
; Raccourci du menu Démarrer, raccourci du Bureau et ouverture des .bbl / .bfl en option, désinstallation
; depuis les Paramètres de Windows.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; Identifiant fixe : ne jamais le changer, il permet les mises à jour par-dessus une version installée.
AppId={{1C99DAAF-8612-4A98-959D-D1289333A103}
AppName=OpenTuneBox
AppVersion={#AppVersion}
AppVerName=OpenTuneBox {#AppVersion}
AppPublisher=Cedric
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\OpenTuneBox
DefaultGroupName=OpenTuneBox
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=OpenTuneBox-Windows-Setup
SetupIconFile=..\opentunebox.ico
UninstallDisplayIcon={app}\OpenTuneBox.exe
UninstallDisplayName=OpenTuneBox
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes
; Fermer OpenTuneBox s'il est ouvert pendant une mise à jour.
CloseApplications=yes

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associate"; Description: "Ouvrir les logs Blackbox (.bbl, .bfl) avec OpenTuneBox"; GroupDescription: "Fichiers :"

[InstallDelete]
; Mise à jour : retirer les bibliothèques de la version précédente, qui peuvent ne plus correspondre.
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "..\..\dist\OpenTuneBox\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\OpenTuneBox"; Filename: "{app}\OpenTuneBox.exe"
Name: "{autodesktop}\OpenTuneBox"; Filename: "{app}\OpenTuneBox.exe"; Tasks: desktopicon

[Registry]
; Type « Log Blackbox » proposé pour .bbl et .bfl (Ouvrir avec), retiré à la désinstallation.
Root: HKA; Subkey: "Software\Classes\.bbl\OpenWithProgids"; ValueType: string; ValueName: "OpenTuneBox.BlackboxLog"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.bfl\OpenWithProgids"; ValueType: string; ValueName: "OpenTuneBox.BlackboxLog"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OpenTuneBox.BlackboxLog"; ValueType: string; ValueName: ""; ValueData: "Log Blackbox Betaflight"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OpenTuneBox.BlackboxLog\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\OpenTuneBox.exe,0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OpenTuneBox.BlackboxLog\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\OpenTuneBox.exe"" ""%1"""; Tasks: associate

[Run]
Filename: "{app}\OpenTuneBox.exe"; Description: "{cm:LaunchProgram,OpenTuneBox}"; Flags: nowait postinstall skipifsilent
