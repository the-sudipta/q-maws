@echo off
rem Q-MAWS launcher for Windows.
rem
rem Looks for the program in bin\ (release bundle), then in target\release\
rem (developer build). If neither exists and cargo is available, builds it.
rem With no arguments, starts the interactive main menu; otherwise passes all
rem arguments through unchanged.
setlocal
set "HERE=%~dp0"
set "PROGRAM="

if exist "%HERE%bin\qmaws.exe" set "PROGRAM=%HERE%bin\qmaws.exe"
if not defined PROGRAM if exist "%HERE%target\release\qmaws.exe" set "PROGRAM=%HERE%target\release\qmaws.exe"

if defined PROGRAM goto run

where cargo >nul 2>nul
if errorlevel 1 goto nocargo

echo Q-MAWS is not built yet. Building it now with cargo (this can take a few minutes).
pushd "%HERE%"
cargo build --release
set "BUILD_STATUS=%errorlevel%"
popd
if not "%BUILD_STATUS%"=="0" (
  echo Error: the build failed. See the messages above.
  exit /b 1
)
if not exist "%HERE%target\release\qmaws.exe" (
  echo Error: the build finished but target\release\qmaws.exe was not found.
  exit /b 1
)
set "PROGRAM=%HERE%target\release\qmaws.exe"
goto run

:nocargo
echo Q-MAWS is not installed in this folder, and Rust (cargo) is not available to build it.
echo.
echo Download the ready-to-run release for Windows from the Releases page of the
echo Q-MAWS repository:
echo   https://github.com/the-sudipta/q-maws/releases
echo Unpack it, then double-click run.bat again in the unpacked folder.
if "%~1"=="" pause
exit /b 1

:run
if "%~1"=="" (
  "%PROGRAM%" menu
) else (
  "%PROGRAM%" %*
)
exit /b %errorlevel%
