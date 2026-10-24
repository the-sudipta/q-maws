@echo off
rem Starts (or continues) the benchmark queue: scripts\benchmark_queue.sh.
rem Double-click this file. To pause, close the window; double-click again
rem later and it continues with the first unfinished job.
title Q-MAWS benchmark queue (close this window to pause)
cd /d "%~dp0.."
set "BASH="
for %%B in ("%ProgramFiles%\Git\usr\bin\bash.exe" "%ProgramFiles%\Git\bin\bash.exe" "E:\DOWNLOADED_SOFTWARES\git\Git\usr\bin\bash.exe") do (
    if not defined BASH if exist %%B set "BASH=%%~B"
)
if not defined BASH (
    echo Git Bash was not found. Install Git for Windows, or edit this file with the path to bash.exe.
    pause
    exit /b 1
)
rem Git Bash's own tools (dirname, date, awk, ...) sit next to bash.exe; put them on the path.
for %%D in ("%BASH%") do set "BASHDIR=%%~dpD"
set "PATH=%BASHDIR%;%SystemRoot%\System32;%SystemRoot%\System32\WindowsPowerShell\v1.0;%PATH%"
echo Q-MAWS benchmark queue
echo Keep the laptop plugged in and on the Performance power mode.
echo To pause: close this window. Double-click start_benchmark.bat again to continue.
echo.
"%BASH%" scripts/benchmark_queue.sh
rem Phase B (the Linux methods) runs after phase A; it skips everything already done.
"%BASH%" scripts/benchmark_queue_b.sh
echo.
pause
