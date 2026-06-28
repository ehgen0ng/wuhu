@echo off
setlocal enabledelayedexpansion
title GreenLuma 2025 x64 Patch Update

echo ==================================================
echo         GreenLuma 2025 x64 Patch Update 6-1-26
echo ==================================================
echo.

set /p "targetDll=Drag and drop GreenLuma_2025_x64.dll here and press enter: "
set "targetDll=%targetDll:"=%"

if not exist "%targetDll%" (
    echo [ERROR] File not found.
    pause
    exit /b
)

set "outputDll=%targetDll%._patched"
copy /y "%targetDll%" "%outputDll%" >nul

echo.
echo [STATUS] Applying 30 byte modifications...

(
echo $path = '%outputDll%'
echo $data = [System.IO.File]::ReadAllBytes($path^)
echo.
echo # Cluster A
echo $data[0x0008AF2D] = 0x00
echo $data[0x0008AF2E] = 0x00
echo.
echo # Cluster B
echo $data[0x0008AF35] = 0x57
echo $data[0x0008AF36] = 0x48
echo $data[0x0008AF37] = 0x83
echo $data[0x0008AF38] = 0xEC
echo $data[0x0008AF39] = 0x30
echo $data[0x0008AF3A] = 0x4D
echo $data[0x0008AF3B] = 0x85
echo $data[0x0008AF3C] = 0xC0
echo $data[0x0008AF3D] = 0x00
echo $data[0x0008AF3E] = 0x00
echo.
echo # InternalGetInt
echo $data[0x0008AF47] = 0x3F
echo $data[0x0008AF57] = 0x00
echo $data[0x0008AF5A] = 0x57
echo.
echo # FamilyGroup Normalization
echo $data[0x0008B22A] = 0x78
echo $data[0x0008B22B] = 0x78
echo $data[0x0008B22C] = 0x78
echo $data[0x0008B22D] = 0x78
echo $data[0x0008B230] = 0x78
echo.
echo # Jump Adjustment
echo $data[0x0008B24A] = 0x9C
echo $data[0x0008B24B] = 0x01
echo $data[0x0008B250] = 0x35
echo.
echo # NOP Cleanup
echo $data[0x0008B255] = 0x0F
echo $data[0x0008B256] = 0x1F
echo $data[0x0008B257] = 0x40
echo $data[0x0008B258] = 0x00
echo $data[0x0008B259] = 0x00
echo $data[0x0008B25A] = 0x00
echo.
echo $data[0x0008B260] = 0x00
echo.
echo [System.IO.File]::WriteAllBytes($path, $data^)
echo Write-Host 'SUCCESS: All 30 offsets patched successfully.' -ForegroundColor Green
) > "%temp%\patcher.ps1"

powershell -NoProfile -ExecutionPolicy Bypass -File "%temp%\patcher.ps1"

del "%temp%\patcher.ps1" >nul 2>&1

echo.
echo [DONE] File saved as:
echo %outputDll%
echo.
pause