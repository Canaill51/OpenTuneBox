@echo off
rem Double-cliquer ce fichier sous Windows pour construire OpenTuneBox.exe
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0construire_windows.ps1"
echo.
pause
