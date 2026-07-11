@echo off
REM TTSBard debug build (double-click or run from a console).
REM Builds the frontend and debug backend executable without installers.
SETLOCAL

REM Change to the repository root (parent of scripts\).
CD /D "%~dp0\.."

REM Bypass the local PowerShell execution policy for this script.
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1" -Mode debug
SET EXITCODE=%ERRORLEVEL%

echo.
if "%EXITCODE%"=="0" (
    echo === Debug build OK ===
) else (
    echo === Debug build FAILED (exit %EXITCODE%) ===
)

ENDLOCAL & EXIT /B %EXITCODE%
