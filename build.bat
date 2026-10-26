@echo off
cd /d "%~dp0"
where node >nul 2>nul || (echo Node.js not found. Install from https://nodejs.org & pause & exit /b 1)
where cargo >nul 2>nul || (echo Rust not found. Install from https://rustup.rs & pause & exit /b 1)
node build-installer.js
echo.
pause
