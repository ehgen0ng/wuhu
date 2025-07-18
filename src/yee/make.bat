@echo off
REM make.bat - Windows make wrapper for yee.exe project

if "%1"=="" goto build
if "%1"=="build" goto build
if "%1"=="run" goto run
if "%1"=="clean" goto clean

echo Available targets: build, run, clean
goto end

:build
echo Compiling C++ source...
g++ -c steam_init.cpp -I./steamworks_sdk_162/sdk/public -o steam_init.o
if errorlevel 1 goto end

echo Copying steam_api64.dll...
copy "steamworks_sdk_162\sdk\redistributable_bin\win64\steam_api64.dll" "steam_api64.dll" >nul
if errorlevel 1 goto end

echo Building yee.exe...
go build -o yee.exe yee.go
if errorlevel 1 goto end

echo Build complete!
goto end

:run
call :build
if exist yee.exe (
    echo Running yee.exe...
    yee.exe
)
goto end

:clean
echo Cleaning...
if exist steam_init.o del steam_init.o
if exist yee.exe del yee.exe
echo Clean complete!
goto end

:end 