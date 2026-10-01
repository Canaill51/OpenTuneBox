# Construit OpenTuneBox pour Windows (à lancer SUR un PC Windows, via construire_windows.bat).
# Étapes : 1) Python 3.13 (installé par winget si absent)  2) environnement .venv-win
#          3) dépendances + PyInstaller  4) dist\OpenTuneBox\OpenTuneBox.exe  5) archive .zip
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Etape($texte) { Write-Host "`n=== $texte ===" -ForegroundColor Cyan }

Etape "1/5 Recherche de Python 3.13"
$py = $null
try { & py -3.13 --version *> $null; if ($LASTEXITCODE -eq 0) { $py = @("py", "-3.13") } } catch {}
if (-not $py) {
    Write-Host "Python 3.13 absent : installation avec winget (une fenêtre d'autorisation peut s'ouvrir)..."
    winget install --id Python.Python.3.13 -e --accept-package-agreements --accept-source-agreements
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")
    $py = @("py", "-3.13")
}
& $py[0] $py[1] --version

Etape "2/5 Environnement Python dédié (.venv-win)"
if (-not (Test-Path ".venv-win\Scripts\python.exe")) { & $py[0] $py[1] -m venv .venv-win }
$vpy = Join-Path $root ".venv-win\Scripts\python.exe"

Etape "3/5 Installation des dépendances (quelques minutes la première fois)"
& $vpy -m pip install --upgrade pip
& $vpy -m pip install . pyinstaller
if ($LASTEXITCODE -ne 0) { throw "échec de l'installation des dépendances" }

Etape "4/5 Fabrication de l'exécutable"
& $vpy -m PyInstaller packaging\opentunebox.spec --noconfirm
if ($LASTEXITCODE -ne 0) { throw "échec de PyInstaller" }

Etape "5/5 Archive à partager"
$zip = Join-Path $root "dist\OpenTuneBox-windows.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "dist\OpenTuneBox" -DestinationPath $zip

Write-Host "`nTerminé." -ForegroundColor Green
Write-Host "Programme : $root\dist\OpenTuneBox\OpenTuneBox.exe"
Write-Host "Archive à partager : $zip"
