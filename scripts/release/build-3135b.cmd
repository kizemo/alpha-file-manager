@echo off
rem Build the packaged installer through the npm tauri:build script.
rem Invoke from anywhere - this script resolves the repo root as %~dp0..\..
rem The toolchain paths below are machine specific. Adjust them if cargo is elsewhere.
setlocal
set "REPO=%~dp0..\.."
set "LOGDIR=%REPO%\.superpowers\sdd"
if not exist "%LOGDIR%" md "%LOGDIR%" >nul 2>&1
set "LOG=%LOGDIR%\tauri-build.log"
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 goto :fail
set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%"
set "CARGO_TERM_COLOR=never"
where cargo
cd /d "%REPO%"
call npm.cmd run tauri:build > "%LOG%" 2>&1
echo TAURI_BUILD_EXIT=%ERRORLEVEL%
exit /b %ERRORLEVEL%
:fail
echo VCVARS_FAILED
exit /b 1