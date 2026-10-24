@echo off
rem Shows the progress of the benchmark queue: finished jobs and the last log lines.
cd /d "%~dp0.."
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command ^
  "$b = 'results\benchmark'; $lock = Join-Path $b 'queue.lock';" ^
  "$running = (Test-Path $lock) -and (Get-Process -Id (Get-Content $lock) -ErrorAction SilentlyContinue);" ^
  "if ($running) { Write-Host 'The benchmark queue is RUNNING.' } else { Write-Host 'The benchmark queue is NOT running. Double-click start_benchmark.bat to start or continue it.' };" ^
  "$v = @(Get-ChildItem (Join-Path $b 'verify') -Filter *.txt -ErrorAction SilentlyContinue).Count;" ^
  "$q = @(Get-ChildItem $b -Recurse -Filter root.txt -ErrorAction SilentlyContinue | Where-Object { $_.DirectoryName -like '*\qmaws\audit' }).Count;" ^
  "$d = @(Get-ChildItem $b -Recurse -Filter done.txt -ErrorAction SilentlyContinue).Count;" ^
  "Write-Host ''; Write-Host ('Phase A progress: ' + ($v + $q + $d) + ' of 213 jobs done  (checks ' + $v + ' of 99, Q-MAWS ' + $q + ' of 30, ML-MAWS and IQ-TREE ' + $d + ' of 84)');" ^
  "Write-Host ''; Write-Host 'Last steps:'; Get-Content (Join-Path $b 'queue.log') -Tail 8"
pause
