; Installateur Windows d'OpenTuneBox (Inno Setup 6), à partir de l'exécutable Rust unique (cargo build --release -p otb-gui,
; fichier rust\target\release\opentunebox-gui.exe), installé sous le nom OpenTuneBox.exe.
;   iscc /DAppVersion=0.9.0 packaging\windows\opentunebox.iss   →   dist\OpenTuneBox-Windows-Setup.exe
;   /DExePath=CHEMIN pour un exécutable ailleurs (le défaut est le dossier de construction de cargo).
; Installation pour l'utilisateur seul par défaut (pas de droits administrateur), ou pour tous au choix.
; Raccourci du menu Démarrer, raccourci du Bureau et ouverture des .bbl / .bfl en option, désinstallation
; depuis les Paramètres de Windows.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef ExePath
  #define ExePath "..\..\rust\target\release\opentunebox-gui.exe"
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
; Licence montrée et acceptée à l'installation : usage non commercial seulement (PolyForm Noncommercial 1.0.0, D-045).
LicenseFile=..\..\LICENSE

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associate"; Description: "Ouvrir les logs Blackbox (.bbl, .bfl) avec OpenTuneBox"; GroupDescription: "Fichiers :"

[InstallDelete]
; Mise à jour par-dessus la version Python (0.8.0 et avant, PyInstaller) : son dossier `_internal` (Python, Qt, numpy…, des
; centaines de Mo) ne sert plus à rien. À retirer quand plus personne n'aura la version Python (W-044).
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; DestName: "OpenTuneBox.exe"; Flags: ignoreversion
; Licence du projet : ses termes doivent accompagner toute copie du programme (section « Notices » de PolyForm Noncommercial, D-045).
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; Mentions de licences des composants embarqués (polices, icônes, bibliothèques) : exigées par leurs licences (W-043).
; Fichier produit par `otb-dev licenses`.
Source: "..\..\THIRD-PARTY-LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion

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
