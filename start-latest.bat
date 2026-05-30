@echo off
setlocal
set "APP_DIR=%~dp0dist\ClipStash"
set "APP_EXE=%APP_DIR%\ClipStash.exe"

if not exist "%APP_EXE%" (
    echo ClipStash.exe not found:
    echo "%APP_EXE%"
    echo.
    echo Please build the app first.
    pause
    exit /b 1
)

start "" "%APP_EXE%"
