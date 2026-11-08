# Construit OpenTuneBox pour Windows (à lancer SUR un PC Windows, via packaging\windows\construire_windows.bat).
# Étapes : 1) Rust (installé par winget si absent)  2) exécutable dist\OpenTuneBox.exe  3) installateur (si Inno Setup 6 est installé)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root

function Etape($texte) { Write-Host "`n=== $texte ===" -ForegroundColor Cyan }

Etape "1/3 Recherche de Rust (cargo)"
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:Path = "$cargoBin;$env:Path" }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust absent : installation avec winget (une fenêtre d'autorisation peut s'ouvrir)..."
    winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements
    $env:Path = "$cargoBin;" + [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo introuvable après l'installation de Rust : rouvrir une fenêtre et relancer" }
}
cargo --version

Etape "2/3 Fabrication de l'exécutable (quelques minutes la première fois)"
# Bibliothèque C embarquée : le programme ne demande aucune DLL de Visual C++ au PC qui l'exécute.
$env:RUSTFLAGS = "-C target-feature=+crt-static"
Push-Location (Join-Path $root "rust")
cargo build --release -p otb-gui
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) {
    throw ("échec de cargo. Si l'erreur parle de « link.exe », installer les outils de compilation C++ de Visual Studio : " +
        "winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override ""--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended""")
}
New-Item -ItemType Directory -Force -Path (Join-Path $root "dist") | Out-Null
$exe = Join-Path $root "dist\OpenTuneBox.exe"
Copy-Item (Join-Path $root "rust\target\release\opentunebox-gui.exe") $exe -Force

Etape "3/3 Installateur (facultatif)"
$meta = cargo metadata --no-deps --format-version 1 --manifest-path (Join-Path $root "rust\Cargo.toml") | ConvertFrom-Json
$version = ($meta.packages | Where-Object { $_.name -eq "otb-gui" }).version
# Sans ce contrôle, une lecture ratée du numéro donnait un installateur « 0.0.0 » sans le moindre avertissement.
if (-not $version) { throw "numéro de version introuvable dans rust\Cargo.toml" }
# Inno Setup 6 installé pour tous les utilisateurs (Program Files) ou « pour moi seulement » (%LOCALAPPDATA%\Programs : dossier
# `{autopf}` en mode non administrateur, déduit du script d'installation officiel is-6_7_4 ; non essayé sur un vrai Windows, W-044).
$iscc = @(
    (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($iscc) {
    & $iscc /Qp "/DAppVersion=$version" (Join-Path $root "packaging\windows\opentunebox.iss")
    if ($LASTEXITCODE -ne 0) { throw "échec d'Inno Setup" }
    Write-Host "Installateur : $root\dist\OpenTuneBox-Windows-Setup.exe"
} else {
    Write-Host "Inno Setup 6 absent : pas d'installateur (winget install --id JRSoftware.InnoSetup). Le programme seul est prêt."
}

Write-Host "`nTerminé (version $version)." -ForegroundColor Green
Write-Host "Programme : $exe"
