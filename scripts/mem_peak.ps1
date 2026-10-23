# Peak memory of a program while it runs: every second, the sum of the working
# sets of all processes with one of the given names; stops when the stop file
# exists (or when the owner process has ended) and writes the peak (MB, whole
# number) to the output file.
# Used by scripts/benchmark_queue.sh (and earlier by the M13 queue).
# -Names takes one name or several separated by commas, e.g. "ml-maws,iqtree2".
param([string]$Names, [string]$Out, [string]$Stop, [int]$Owner = 0)
$list = $Names -split ','
$peak = 0
while (-not (Test-Path $Stop)) {
    $sum = (Get-Process -Name $list -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum).Sum
    if ($sum -gt $peak) { $peak = $sum }
    if ($Owner -gt 0 -and -not (Get-Process -Id $Owner -ErrorAction SilentlyContinue)) { break }
    Start-Sleep -Seconds 1
}
[math]::Round($peak / 1MB) | Set-Content -Path $Out -Encoding ascii
