@echo off
setlocal
set "NEXT_DIR=%~dp0clipstash-next"

if not exist "%NEXT_DIR%\package.json" (
    echo ClipStash Next project not found:
    echo "%NEXT_DIR%"
    echo.
    pause
    exit /b 1
)

where npm >nul 2>nul
if errorlevel 1 (
    echo npm not found. Please install Node.js or build ClipStash Next first:
    echo "%NEXT_DIR%"
    echo.
    pause
    exit /b 1
)

cd /d "%NEXT_DIR%"
npm run tauri dev
