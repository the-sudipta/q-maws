@echo off
rem Install the Q-MAWS pre-commit guard as the pre-commit and commit-msg hooks
rem of the current repository. Git for Windows runs the hooks with its bundled
rem shell. Run from anywhere inside the repository.
setlocal
set "ROOT="
for /f "delims=" %%i in ('git rev-parse --show-toplevel') do set "ROOT=%%i"
if not defined ROOT (
  echo Error: this folder is not inside a git repository.
  exit /b 1
)
for /f "delims=" %%i in ('git rev-parse --git-path hooks') do set "HOOKS=%%i"
set "ROOT=%ROOT:/=\%"
set "HOOKS=%HOOKS:/=\%"
if not exist "%HOOKS%" mkdir "%HOOKS%"
for %%h in (pre-commit commit-msg) do (
  copy /y "%ROOT%\scripts\pre-commit" "%HOOKS%\%%h" >nul || exit /b 1
  echo Installed %HOOKS%\%%h
)
endlocal
