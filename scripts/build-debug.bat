@echo off
REM build-debug.bat — debug-сборка TTSBard (двойной клик или консоль).
REM Компилирует бэкенд в debug-профайле + фронтенд, выдаёт runnable ttsbard.exe.
REM Инсталляторы (nsis/msi) НЕ собираются — для этого используйте build-release.bat.
SETLOCAL

REM Переход в корень репо (родитель папки scripts\).
CD /D "%~dp0\.."

REM -ExecutionPolicy Bypass — обходит локальную политику выполнения PS.
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1" -Mode debug
SET EXITCODE=%ERRORLEVEL%

echo.
if "%EXITCODE%"=="0" (
    echo === Debug build OK ===
) else (
    echo === Debug build FAILED (exit %EXITCODE%) ===
)

ENDLOCAL & EXIT /B %EXITCODE%
