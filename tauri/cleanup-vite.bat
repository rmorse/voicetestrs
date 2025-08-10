@echo off
REM Cleanup script for Vite dev server
echo Cleaning up Vite processes...

REM Find and kill any node processes running vite
for /f "tokens=2" %%i in ('tasklist ^| findstr "node.exe"') do (
    wmic process where ProcessId=%%i get CommandLine 2>nul | findstr /i "vite" >nul
    if not errorlevel 1 (
        echo Killing Vite process with PID %%i
        taskkill /PID %%i /F /T >nul 2>&1
    )
)

REM Clean up port file
if exist ".current-port" (
    del ".current-port"
    echo Cleaned up port file
)

echo Cleanup complete