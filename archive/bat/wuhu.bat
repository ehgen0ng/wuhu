@echo off & setlocal enabledelayedexpansion

if not exist "%~dp0utils\GreenLuma\AppList" (
        mkdir "%~dp0utils\GreenLuma\AppList\"
    ) else (
        del /Q "%~dp0utils\GreenLuma\AppList\"
    )

set n=0
set "added_ids="
for %%i in ("%~dp0List\*.TXT") do (
for /f "usebackq delims=" %%j in ("%%i") do (
        echo !added_ids! | find "%%j" >nul
        if errorlevel 1 (
            echo/%%~j>"%~dp0utils\GreenLuma\AppList\"^!n^!.txt
            set "added_ids=!added_ids! %%j"
            set /a n+=1
            if exist "%~dp0utils\ManifestHub\%%j\%%j.txt" (
                for /f "usebackq delims=" %%k in ("%~dp0utils\ManifestHub\%%j\%%j.txt") do (
                    echo !added_ids! | find "%%k" >nul
                    if errorlevel 1 (
                        echo/%%k>>"%~dp0utils\GreenLuma\AppList\"^!n^!.txt
                        set "added_ids=!added_ids! %%k"
                        set /a n+=1
                    )
                )
            )
        )
    )
)


:start
taskkill /F /IM steam.exe >nul 2>&1
cd /d "%~dp0utils\GreenLuma"
DeleteSteamAppCache.exe
DLLInjector.exe

exit
