@echo off
setlocal
cd /d "%~dp0"

echo First-time Brokenithm setup
echo This may install Node.js, Rust, and Microsoft C++ build tools.
echo.

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup-brokenithm.ps1" -Action All -InstallPrerequisites
set "RESULT=%ERRORLEVEL%"

echo.
if not "%RESULT%"=="0" (
  echo Setup did not complete. Read the error above, then run this file again.
) else (
  echo Setup complete. Use start-brokenithm.cmd next time.
)
pause
exit /b %RESULT%
