@echo off
rem Tamper check of the benchmark results: seals every newly finished run with
rem SHA-256, then checks every sealed run file by file and the overall manifest
rem (results\benchmark\benchmark.sha256 with its root fingerprint).
rem PASS means no sealed file was changed, removed or added.
title Q-MAWS benchmark: verify the results
cd /d "%~dp0.."
echo Sealing newly finished runs...
python scripts\bench_hash.py seal
echo.
echo Checking every sealed run...
python scripts\bench_hash.py verify
echo.
pause
