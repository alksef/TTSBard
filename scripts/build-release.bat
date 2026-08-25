@echo off
REM build-release.bat — релизная сборка TTSBard (двойной клик или консоль).
REM Полная сборка: оптимизированный ttsbard.exe + инсталляторы (nsis/msi).
SETLOCAL

REM Переход в корень репо (родитель папки scripts\).
CD /D "%~dp0\.."

REM Prefer PowerShell 7 when installed, but keep the wrapper usable on a
REM standard Windows installation that only has Windows PowerShell.
where pwsh >nul 2>nul
if %ERRORLEVEL% EQU 0 (
    pwsh -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1" -Mode release
) else (
    "%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1" -Mode release
)
SET EXITCODE=%ERRORLEVEL%

echo.
if "%EXITCODE%"=="0" (
    echo === Release build OK ===
) else (
    echo === Release build FAILED (exit %EXITCODE%) ===
)

ENDLOCAL & EXIT /B %EXITCODE%
