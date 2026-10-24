@echo off
rem Opens the live dashboard of the benchmark: state, current job and its end,
rem overall progress, the time left and when everything ends (local time,
rem 12-hour clock), speed, processor, memory, disk, power, and the last steps.
rem It updates itself; press Q to close it. The benchmark keeps running.
title Q-MAWS benchmark: live status (press Q to close)
cd /d "%~dp0.."
mode con: cols=114 lines=46 >nul 2>&1
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0benchmark_dashboard.ps1"
