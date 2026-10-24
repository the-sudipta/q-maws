@echo off
rem Pauses the benchmark queue: stops the queue, every process it started, and
rem the program it is running. Nothing is lost: double-click
rem start_benchmark.bat later and it continues with the first unfinished job.
cd /d "%~dp0.."
set "PS=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if exist "results\benchmark\queue.lock" (
    set /p QPID=<"results\benchmark\queue.lock"
)
if defined QPID (
    taskkill /PID %QPID% /T /F >nul 2>&1
    del /q "results\benchmark\queue.lock" >nul 2>&1
)
rem Phase B runs its tools inside Ubuntu (WSL): stop them too.
wsl.exe --terminate Ubuntu-24.04 >nul 2>&1
"%PS%" -NoProfile -Command "Get-Process -Name qmaws,ml-maws,iqtree2 -ErrorAction SilentlyContinue | Stop-Process -Force; Write-Host 'The benchmark queue is paused. Double-click start_benchmark.bat to continue.'"
pause
