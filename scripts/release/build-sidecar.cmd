@echo off
rem Build the focus-sync sidecar crate and copy the binary into
rem extensions\kizemo.focus-sync\bin and extensions\kizemo.focus-sync\installer.
rem Invoke from anywhere - this script resolves the repo root as %~dp0..\..
rem The toolchain paths below are machine specific. Adjust them if cargo is elsewhere.
setlocal
set "REPO=%~dp0..\.."
set "SIDECAR=%REPO%\extensions\kizemo.focus-sync\sidecar"
set "LOGDIR=%REPO%\.superpowers\sdd"
if not exist "%LOGDIR%" md "%LOGDIR%" >nul 2>&1
set "LOG=%LOGDIR%\build-sidecar.log"
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%"
cd /d "%SIDECAR%"
if errorlevel 1 goto fail
cargo build --release > "%LOG%" 2>&1
if errorlevel 1 goto fail
copy /Y target\release\spike.exe ..\bin\focus-sync-sidecar.exe >nul
copy /Y target\release\spike.exe ..\installer\focus-sync-sidecar.exe >nul
echo BUILD_OK >> "%LOG%"
exit /b 0
:fail
echo BUILD_FAILED >> "%LOG%"
exit /b 1