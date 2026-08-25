@echo off
REM TTSBard debug build (double-click or run from a console).
REM Builds the frontend and debug backend executable without installers.
SETLOCAL

REM Change to the repository root (parent of scripts\).
CD /D "%~dp0\.."

REM Prefer PowerShell 7 when installed, but keep the wrapper usable on a
REM standard Windows installation that only has Windows PowerShell.
where pwsh >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    pwsh -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1" -Mode debug
) else (
    "%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1" -Mode debug
)
SET EXITCODE=%ERRORLEVEL%

echo.
if "%EXITCODE%"=="0" (
    echo === Debug build OK ===
) else (
    echo === Debug build FAILED (exit %EXITCODE%) ===
)

ENDLOCAL & EXIT /B %EXITCODE%
