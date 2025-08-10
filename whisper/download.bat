@echo off
REM Whisper Binary Downloader for Windows
REM Downloads and extracts whisper.cpp binaries

setlocal enabledelayedexpansion

echo ============================================
echo Whisper Binary Downloader for Windows
echo ============================================
echo.

REM Default to CPU version if no argument provided
set "VERSION=%1"
if "%VERSION%"=="" set "VERSION=cpu"

REM Configuration - paths relative to whisper directory
set "WHISPER_DIR=%~dp0"
set "RELEASE_DIR=%WHISPER_DIR%Release"

REM URLs for different versions (as of late 2024)
REM Users should check https://github.com/Purfview/whisper-standalone-win/releases for latest versions
set "CPU_URL=https://github.com/Purfview/whisper-standalone-win/releases/download/r245.4/Whisper-Faster-XXL-main.zip"
set "CUDA_URL=https://github.com/Purfview/whisper-standalone-win/releases/download/r245.4/Whisper-Faster-XXL-cuda11.zip"
set "CUDA12_URL=https://github.com/Purfview/whisper-standalone-win/releases/download/r245.4/Whisper-Faster-XXL-cuda12.zip"

REM Select URL based on version
if /i "%VERSION%"=="cpu" (
    set "DOWNLOAD_URL=%CPU_URL%"
    set "VERSION_NAME=CPU"
) else if /i "%VERSION%"=="cuda" (
    set "DOWNLOAD_URL=%CUDA_URL%"
    set "VERSION_NAME=CUDA 11"
) else if /i "%VERSION%"=="cuda11" (
    set "DOWNLOAD_URL=%CUDA_URL%"
    set "VERSION_NAME=CUDA 11"
) else if /i "%VERSION%"=="cuda12" (
    set "DOWNLOAD_URL=%CUDA12_URL%"
    set "VERSION_NAME=CUDA 12"
) else (
    echo Error: Unknown version "%VERSION%"
    echo.
    echo Usage: download.bat [version]
    echo.
    echo Available versions:
    echo   cpu     - CPU-only version (default)
    echo   cuda    - CUDA 11 version for NVIDIA GPUs
    echo   cuda11  - CUDA 11 version for NVIDIA GPUs
    echo   cuda12  - CUDA 12 version for NVIDIA GPUs
    echo.
    echo Example: download.bat cuda12
    echo.
    echo Note: For latest URLs, check:
    echo https://github.com/Purfview/whisper-standalone-win/releases
    exit /b 1
)

echo Selected version: %VERSION_NAME%
echo.

REM Check if whisper directory exists and has files
if exist "%RELEASE_DIR%\whisper-cli.exe" (
    echo Whisper binaries already exist in: %RELEASE_DIR%
    set /p "OVERWRITE=Do you want to re-download and overwrite? (y/n): "
    if /i not "!OVERWRITE!"=="y" (
        echo Installation cancelled.
        exit /b 0
    )
)

REM Create Release directory if it doesn't exist
if not exist "%RELEASE_DIR%" mkdir "%RELEASE_DIR%"

REM Download the file
set "TEMP_ZIP=%TEMP%\whisper-download.zip"
echo Downloading Whisper binaries...
echo URL: %DOWNLOAD_URL%
echo.

REM Use PowerShell to download
powershell -Command "& { try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '%DOWNLOAD_URL%' -OutFile '%TEMP_ZIP%' -UseBasicParsing } catch { Write-Host 'Download failed:' $_.Exception.Message -ForegroundColor Red; exit 1 } }"

if errorlevel 1 (
    echo.
    echo Error: Failed to download Whisper binaries.
    echo Please check your internet connection or download manually from:
    echo %DOWNLOAD_URL%
    exit /b 1
)

echo Download complete. Extracting...
echo.

REM Extract the ZIP file using PowerShell
powershell -Command "& { try { Expand-Archive -Path '%TEMP_ZIP%' -DestinationPath '%TEMP%\whisper-extract' -Force } catch { Write-Host 'Extraction failed:' $_.Exception.Message -ForegroundColor Red; exit 1 } }"

if errorlevel 1 (
    echo.
    echo Error: Failed to extract the ZIP file.
    del "%TEMP_ZIP%" 2>nul
    exit /b 1
)

REM Move files to the Release directory
echo Moving files to %RELEASE_DIR%...
xcopy /E /Y "%TEMP%\whisper-extract\*" "%RELEASE_DIR%\" >nul 2>&1

REM Clean up temporary files
del "%TEMP_ZIP%" 2>nul
rd /s /q "%TEMP%\whisper-extract" 2>nul

REM Verify installation
if exist "%RELEASE_DIR%\whisper-cli.exe" (
    echo.
    echo ============================================
    echo Installation successful!
    echo ============================================
    echo.
    echo Whisper binaries installed to: %RELEASE_DIR%
    echo.
    echo Next steps:
    echo 1. Download a model: download-model.bat base.en
    echo 2. Test transcription: Release\whisper-cli.exe -m models\ggml-base.en.bin audio.wav
    echo.
) else if exist "%RELEASE_DIR%\main.exe" (
    echo.
    echo ============================================
    echo Installation successful!
    echo ============================================
    echo.
    echo Note: This appears to be an older version using main.exe
    echo Consider updating to a newer release that uses whisper-cli.exe
    echo.
    echo Whisper binaries installed to: %RELEASE_DIR%
    echo.
) else (
    echo.
    echo Warning: Installation may not be complete.
    echo Could not find whisper-cli.exe or main.exe in %RELEASE_DIR%
    echo Please check the directory manually.
)

endlocal