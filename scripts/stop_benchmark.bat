@echo off
rem Pauses the benchmark queue: stops the queue and the program it is running.
rem Nothing is lost: double-click start_benchmark.bat later to continue.
cd /d "%~dp0.."
powershell -NoProfile -Command ^
  "$lock = 'results\benchmark\queue.lock';" ^
  "if (Test-Path $lock) { $id = Get-Content $lock; Stop-Process -Id $id -Force -ErrorAction SilentlyContinue; Remove-Item $lock -ErrorAction SilentlyContinue };" ^
  "Get-Process -Name qmaws,ml-maws,iqtree2 -ErrorAction SilentlyContinue | Stop-Process -Force;" ^
  "Write-Host 'The benchmark queue is paused. Double-click start_benchmark.bat to continue.'"
pause
