@echo off
REM Wrapper batch file for downloading Whisper models

if "%1"=="" (
    echo Usage: download-model.bat [model-name]
    echo.
    echo Example: download-model.bat base.en
    echo          download-model.bat small
    echo          download-model.bat large-v3
    echo.
    echo Run without arguments to see all available models.
    powershell -ExecutionPolicy Bypass -File "%~dp0download-model.ps1"
) else (
    powershell -ExecutionPolicy Bypass -File "%~dp0download-model.ps1" -Model %1
)