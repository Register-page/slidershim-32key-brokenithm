@echo off
setlocal
cd /d "%~dp0"

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup-brokenithm.ps1" -Action Run
if errorlevel 1 pause
