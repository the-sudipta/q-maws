# Live dashboard of the extended benchmark (scripts/benchmark_status.bat).
# Redraws in place every few seconds: queue state, the current job with its
# estimated end, overall progress of phases A and B with the estimated end of
# everything, speed, computer resources, and the last log lines. All times are
# shown in the computer's local time, 12-hour format. Estimates use the median
# wall time of finished runs of the same method (results/benchmark/runs.tsv),
# or a first guess until such runs exist. Press Q to close; the queue keeps running.
# -Once prints one frame and exits (for tests).
param([switch]$Once)

$ErrorActionPreference = 'SilentlyContinue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$B = Join-Path $root 'results\benchmark'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$esc = [char]27

# colours: switch on ANSI escape codes in the classic console window too
try {
    Add-Type -Namespace Q -Name Con -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetStdHandle(int h); [DllImport("kernel32.dll")] public static extern bool GetConsoleMode(System.IntPtr h, out uint m); [DllImport("kernel32.dll")] public static extern bool SetConsoleMode(System.IntPtr h, uint m);'
    $h = [Q.Con]::GetStdHandle(-11); $mode = 0
    if ([Q.Con]::GetConsoleMode($h, [ref]$mode)) { [void][Q.Con]::SetConsoleMode($h, $mode -bor 4) }
} catch {}
function C([string]$t, [string]$code) { "$esc[${code}m$t$esc[0m" }
$tf = 'hh:mm:ss tt'
$df = 'ddd d MMM, hh:mm tt'

# --- the job list (same as the queue scripts) -------------------------------
$NEW = @('bm_div005','bm_div020','bm_div050','bm_m15','bm_m60','bm_frag') | ForEach-Object { $p = $_; 1..5 | ForEach-Object { "${p}_r$_" } }
$H5 = @('h000','h010','h020','h040') | ForEach-Object { $l = $_; 1..5 | ForEach-Object { "hgt_${l}_r$_" } }
$AF = 'fish_mito','ecoli','ecoli_shigella_hgt','yersinia_hgt','sim_hgt_0','sim_hgt_250','sim_hgt_500','sim_hgt_750','sim_hgt_1000'
$TUNE = 1..6 | ForEach-Object { "bm_tune_r$_" }
$SPEED = 25,50,100 | ForEach-Object { $m = $_; 1..3 | ForEach-Object { "scaling_m${m}_r$_" } }
$TOOLS = 'mash','cdmaws','peafowl','andi','phylonium','fswm','multispam','sans','cvtree','jolytree','skmer','mashtree'
$GRID = @{ mash = 15,17,19,21,23,25,31; mashtree = 15,17,19,21,23,25,31; sans = 15,19,23,27,31; skmer = 15,19,23,27,31; fswm = 8,10,12,14,16; multispam = 8,10,12; cvtree = 7,9,11,13 }
$DEFAULT = @{ mash = 21; mashtree = 21; sans = 31; skmer = 31; fswm = 12; multispam = 10; cvtree = 7 }
# first guesses (seconds, a 30-species dataset) until real runs exist
$PRIOR = @{ qmaws = 3600; ml_maws = 1800; iqtree_true = 1800; verify = 60; mash = 10; cdmaws = 90; peafowl = 2400; andi = 15; phylonium = 10; fswm = 30; multispam = 2400; sans = 30; cvtree = 15; jolytree = 90; skmer = 30; mashtree = 15 }
$NICE = @{ qmaws = 'Q-MAWS'; ml_maws = 'ML-MAWS'; iqtree_true = 'IQ-TREE reference'; mash = 'Mash'; cdmaws = 'CD-MAWS'; peafowl = 'Peafowl'; andi = 'andi'; phylonium = 'phylonium'; fswm = 'FSWM'; multispam = 'Multi-SpaM'; sans = 'SANS'; cvtree = 'CVTree'; jolytree = 'JolyTree'; skmer = 'Skmer'; mashtree = 'mashtree' }

function Fit([string]$s, [int]$w) {
    $sb = New-Object System.Text.StringBuilder; $vis = 0; $i = 0
    while ($i -lt $s.Length) {
        if ($s[$i] -eq $esc) { $j = $s.IndexOf('m', $i); if ($j -lt 0) { break }; [void]$sb.Append($s.Substring($i, $j - $i + 1)); $i = $j + 1; continue }
        if ($vis -ge $w) { break }
        [void]$sb.Append($s[$i]); $vis++; $i++
    }
    [void]$sb.Append("$esc[0m"); $sb.ToString()
}
function Leave { [Console]::Write("$esc[?25h$esc[?1049l") }

function Has([string]$rel) { Test-Path (Join-Path $B $rel) }
function Marked([string]$rel) { (Has "$rel\done.txt") -or (Has "$rel\failed.txt") }

function Get-Remaining {
    # returns a list of [method] for every job not yet done, and counts per phase
    $rem = New-Object System.Collections.Generic.List[string]
    $a = @{ done = 0; total = 0 }; $bt = @{ done = 0; total = 0 }; $be = @{ done = 0; total = 0 }; $bs = @{ done = 0; total = 0 }
    $vd = @(Get-ChildItem (Join-Path $B 'verify') -Filter *.txt).Count
    $a.total += 99; $a.done += [Math]::Min($vd, 99); 1..([Math]::Max(0, 99 - $vd)) | Where-Object { $_ -gt 0 } | ForEach-Object { $rem.Add('verify') }
    foreach ($id in $NEW) {
        $a.total++; if (Has "$id\qmaws\audit\root.txt") { $a.done++ } else { $rem.Add('qmaws') }
        $a.total++; if (Has "$id\ml_maws\done.txt") { $a.done++ } else { $rem.Add('ml_maws') }
        if ($id -notlike 'bm_frag_*') { $a.total++; if (Has "$id\iqtree_true\done.txt") { $a.done++ } else { $rem.Add('iqtree_true') } }
    }
    foreach ($id in $AF) { $a.total++; if (Has "$id\ml_maws\done.txt") { $a.done++ } else { $rem.Add('ml_maws') } }
    foreach ($id in $H5) { $a.total++; if (Has "$id\iqtree_true\done.txt") { $a.done++ } else { $rem.Add('iqtree_true') } }
    foreach ($m in $GRID.Keys) { foreach ($v in $GRID[$m]) { foreach ($ds in $TUNE) {
        $bt.total++; if (Marked "tuning\${m}_$v\$ds") { $bt.done++ } else { $rem.Add($m) } } } }
    $chosen = @{}
    $ct = Join-Path $B 'tuning\chosen.tsv'
    if (Test-Path $ct) { Get-Content $ct | Select-Object -Skip 1 | ForEach-Object { $f = $_ -split "`t"; $chosen[$f[0]] = ($f[2] -ne $f[1]) } }
    foreach ($ds in ($AF + $H5 + $NEW)) { foreach ($m in $TOOLS) {
        $be.total++; if (Marked "$ds\$m") { $be.done++ } else { $rem.Add($m) }
        $tuned = if ($chosen.ContainsKey($m)) { $chosen[$m] } else { $DEFAULT.ContainsKey($m) }   # until tuning ends: assume a tuned run
        if ($tuned) { $be.total++; if (Marked "$ds\${m}_tuned") { $be.done++ } else { $rem.Add($m) } }
    } }
    foreach ($ds in $SPEED) { foreach ($m in $TOOLS) { $bs.total++; if (Marked "speed\$ds\$m") { $bs.done++ } else { $rem.Add($m) } } }
    $failed = @(Get-ChildItem $B -Recurse -Depth 4 -Filter failed.txt).Count
    [pscustomobject]@{ Rem = $rem; A = $a; BT = $bt; BE = $be; BS = $bs; Failed = $failed; Tuned = (Test-Path $ct) }
}

function Get-Medians {
    $med = @{}
    $runs = Join-Path $B 'runs.tsv'
    if (-not (Test-Path $runs)) { return $med }
    $by = @{}
    Get-Content $runs | Select-Object -Skip 1 | ForEach-Object {
        $f = $_ -split "`t"
        if ($f.Count -ge 7 -and $f[6] -eq '0' -and $f[4] -match '^\d+$') {
            $m = $f[1] -replace '_\d+$', ''
            if (-not $by.ContainsKey($m)) { $by[$m] = New-Object System.Collections.Generic.List[double] }
            $by[$m].Add([double]$f[4])
        }
    }
    foreach ($k in $by.Keys) { $s = $by[$k] | Sort-Object; $n = $s.Count; $med[$k] = if ($n % 2) { $s[[int][Math]::Floor($n / 2)] } else { ($s[$n / 2 - 1] + $s[$n / 2]) / 2 } }
    $med
}

function Est([string]$m, $med) { if ($med.ContainsKey($m)) { $med[$m] } else { $PRIOR[$m] } }
function Dur([double]$s) {
    if ($s -lt 0) { $s = 0 }
    $t = [TimeSpan]::FromSeconds([Math]::Round($s))
    if ($t.TotalDays -ge 1) { '{0}d {1}h {2:00}m' -f [int][Math]::Floor($t.TotalDays), $t.Hours, $t.Minutes }
    elseif ($t.TotalHours -ge 1) { '{0}h {1:00}m' -f $t.Hours, $t.Minutes }
    else { '{0}m {1:00}s' -f $t.Minutes, $t.Seconds }
}
function Bar([double]$frac, [int]$w, [string]$col) {
    $frac = [Math]::Max([double]0, [Math]::Min([double]1, $frac)); $n = [int][Math]::Round($frac * $w)
    (C ('█' * $n) $col) + (C ('░' * ($w - $n)) '90')
}
function UtcToLocal([string]$s) { ([datetime]::Parse($s, $null, [Globalization.DateTimeStyles]::AdjustToUniversal -bor [Globalization.DateTimeStyles]::AssumeUniversal)).ToLocalTime() }

function Get-Current {
    # the last start line of the log that has no result after it
    $log = Join-Path $B 'queue.log'
    if (-not (Test-Path $log)) { return $null }
    $last = Get-Content $log -Tail 1
    if (-not $last) { return $null }
    $ts = $last.Substring(0, 20); $msg = $last.Substring(21)
    $m = $null; $ds = $null
    if ($msg -match '^Q-MAWS (\S+)( \(continue\))?$') { $m = 'qmaws'; $ds = $Matches[1] }
    elseif ($msg -match '^ML-MAWS (\S+)$') { $m = 'ml_maws'; $ds = $Matches[1] }
    elseif ($msg -match '^IQ-TREE reference (\S+)$') { $m = 'iqtree_true'; $ds = $Matches[1] }
    elseif ($msg -match '^verify (\S+)$') { $m = 'verify'; $ds = $Matches[1] }
    elseif ($msg -match '^(\w+)( \((\d+)\))? on (\S+)$') { $m = $Matches[1]; $ds = $Matches[4]; if ($Matches[3]) { $ds = "$ds  (setting $($Matches[3]))" } }
    if (-not $m) { return $null }
    [pscustomobject]@{ Method = $m; Dataset = $ds; Start = (UtcToLocal $ts) }
}

function Get-QueueStart {
    $l = Select-String -Path (Join-Path $B 'queue.log') -Pattern 'queue started|phase B started' | Select-Object -Last 1
    if ($l) { UtcToLocal $l.Line.Substring(0, 20) } else { $null }
}

# --- slow-changing values are cached --------------------------------------
$cache = @{ t = [datetime]::MinValue; slow = [datetime]::MinValue }
$spin = '|','/','-','\'
$tick = 0
if (-not $Once) { [Console]::Write("$esc[?1049h$esc[?25l$esc[2J$esc[3;3H" + (C 'Q-MAWS benchmark: loading the status, one moment...' '96')) }
$cache.sizeT = [datetime]::MinValue

while ($true) {
    $now = Get-Date
    if (($now - $cache.t).TotalSeconds -ge 15 -or $Once) {
        $cache.R = Get-Remaining; $cache.Med = Get-Medians; $cache.t = $now
        $lockf = Join-Path $B 'queue.lock'
        $cache.Running = (Test-Path $lockf) -and ($p = Get-Process -Id (Get-Content $lockf) -ErrorAction SilentlyContinue) -and ($p.Name -in 'sh', 'bash')
        $cache.Cur = Get-Current; $cache.QStart = Get-QueueStart
        $cache.Done24 = @(Get-Content (Join-Path $B 'runs.tsv') | Select-Object -Skip 1 | Where-Object { $f = $_ -split "`t"; $f.Count -ge 4 -and ((UtcToLocal $f[3]) -gt $now.AddHours(-24)) }).Count
        $cache.Log = Get-Content (Join-Path $B 'queue.log') -Tail 6
    }
    if (($now - $cache.slow).TotalSeconds -ge 30 -or $Once) {
        $cpu = (Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average).Average
        $os = Get-CimInstance Win32_OperatingSystem
        $cache.Cpu = [int]$cpu
        $cache.MemUsed = ($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / 1MB; $cache.MemTot = $os.TotalVisibleMemorySize / 1MB
        $d = Get-PSDrive D; $cache.DiskFree = $d.Free / 1GB; $cache.DiskTot = ($d.Used + $d.Free) / 1GB
        if (($now - $cache.sizeT).TotalSeconds -ge 300 -or $Once) { $cache.Size = (Get-ChildItem $B -Recurse -File | Measure-Object Length -Sum).Sum / 1GB; $cache.sizeT = $now }
        $bat = Get-CimInstance Win32_Battery
        $cache.Bat = if ($bat) { [int]$bat.EstimatedChargeRemaining } else { $null }
        $cache.AC = if ($bat) { $bat.BatteryStatus -eq 2 -or $bat.BatteryStatus -ge 6 } else { $true }
        $cache.slow = $now
    }
    $R = $cache.R; $med = $cache.Med
    $W = [Math]::Max(60, [Math]::Min(110, $Host.UI.RawUI.WindowSize.Width - 2))
    $bw = [Math]::Max(20, $W - 46)
    $L = New-Object System.Collections.Generic.List[string]
    $sp = $spin[$tick % $spin.Count]; $tick++

    $state = if ($cache.Running) { C "$sp RUNNING" '1;32' } else { C '■ NOT RUNNING' '1;31' }
    $L.Add((C '  Q-MAWS BENCHMARK  ' '1;97;44') + '   ' + $state + '      ' + (C ($now.ToString($df)) '96') + '  ' + (C ($now.ToString('ss')) '90') + 's')
    $L.Add((C ('─' * $W) '90'))
    if ($cache.Running -and $cache.QStart) { $L.Add('  Queue running since  ' + (C ($cache.QStart.ToString($df)) '97') + '   (' + (Dur ($now - $cache.QStart).TotalSeconds) + ')') }
    elseif (-not $cache.Running) { $L.Add('  ' + (C 'Paused or finished. Double-click start_benchmark.bat to start or continue.' '93')) }
    else { $L.Add('') }
    $L.Add('')

    # current job
    $L.Add((C '  CURRENT JOB' '1;97'))
    $cur = $cache.Cur
    if ($cache.Running -and $cur) {
        $est = Est $cur.Method $med
        $el = ($now - $cur.Start).TotalSeconds
        $frac = if ($est -gt 0) { [Math]::Min(0.99, $el / $est) } else { 0 }
        $endc = $cur.Start.AddSeconds([Math]::Max($est, $el + 60))
        $L.Add('  ' + (C $(if ($NICE.ContainsKey($cur.Method)) { $NICE[$cur.Method] } else { $cur.Method }) '1;96') + '  on  ' + (C $cur.Dataset '97'))
        $L.Add('  Started  ' + $cur.Start.ToString($tf) + '    running ' + (C (Dur $el) '97') + '    typical ' + (Dur $est))
        if ($el -le $est) {
            $L.Add('  ' + (Bar $frac $bw '36') + '  ' + ('{0,3:N0}%' -f ($frac * 100)) + '   ends about ' + (C ($endc.ToString($tf)) '1;97'))
        } else {
            $L.Add('  ' + (Bar 1 $bw '33') + '  ' + (C ('running longer than usual (+' + (Dur ($el - $est)) + ')') '33'))
        }
        if ($cur.Method -eq 'qmaws') {
            $ql = Get-Content (Join-Path $B "$($cur.Dataset)\qmaws
un.log") -Tail 1
            if ($ql -and $ql.Length -gt 21) { $qt = 'Q-MAWS: ' + $ql.Substring(21); if ($qt.Length -gt $W - 4) { $qt = $qt.Substring(0, $W - 7) + '...' }; $L.Add('  ' + (C $qt '90')) } else { $L.Add('') }
        } elseif ($cur.Method -eq 'iqtree_true') {
            $il = Get-Content (Join-Path $B "$($cur.Dataset)\iqtree_true\iqtree.out.txt") -Tail 30 | Where-Object { $_ -match 'left\)|ModelFinder|Iteration' } | Select-Object -Last 1
            if ($il) { $it = 'IQ-TREE: ' + $il.Trim(); if ($it.Length -gt $W - 4) { $it = $it.Substring(0, $W - 7) + '...' }; $L.Add('  ' + (C $it '90')) } else { $L.Add('') }
        } else { $L.Add('') }
    } else { $L.Add('  ' + (C 'between jobs' '90')); $L.Add(''); $L.Add(''); $L.Add('') }
    $L.Add('')

    # overall progress
    $L.Add((C '  OVERALL PROGRESS' '1;97'))
    $rows = @(@('Phase A  Windows methods', $R.A, '32'), @('Phase B  tuning', $R.BT, '33'), @('Phase B  evaluation', $R.BE, '35'), @('Phase B  speed', $R.BS, '34'))
    foreach ($row in $rows) {
        $fr = if ($row[1].total) { $row[1].done / $row[1].total } else { 0 }
        $L.Add(('  {0,-26}' -f $row[0]) + (Bar $fr $bw $row[2]) + ('  {0,5:N1}%  {1,4}/{2,-4}' -f ($fr * 100), $row[1].done, $row[1].total))
    }
    $dn = $R.A.done + $R.BT.done + $R.BE.done + $R.BS.done; $tt = $R.A.total + $R.BT.total + $R.BE.total + $R.BS.total
    $L.Add('  ' + (C ('{0,-26}' -f 'All jobs') '1;97') + (Bar ($dn / [Math]::Max(1, $tt)) $bw '1;32') + ('  {0,5:N1}%  {1,4}/{2,-4}' -f (100 * $dn / [Math]::Max(1, $tt)), $dn, $tt))
    if (-not $R.Tuned) { $L.Add('  ' + (C 'Totals include a tuned run for every tunable method until tuning has chosen the values.' '90')) } else { $L.Add('') }
    $L.Add('')

    # time
    $remS = 0; foreach ($m in $R.Rem) { $remS += (Est $m $med) }
    if ($cache.Running -and $cur) { $remS = [Math]::Max(0, $remS - ($now - $cur.Start).TotalSeconds) }
    $eta = $now.AddSeconds($remS)
    $L.Add((C '  TIME' '1;97'))
    $L.Add('  Work left            ' + (C (Dur $remS) '1;97') + '   (' + $R.Rem.Count + ' jobs, if the laptop runs without breaks)')
    $L.Add('  Everything done at   ' + (C ($eta.ToString($df)) '1;92') + '   (local time)')
    $L.Add('  Jobs finished in the last 24 h:  ' + $cache.Done24 + '      failed so far:  ' + $(if ($R.Failed) { C $R.Failed '1;31' } else { C '0' '32' }))
    $L.Add('')

    # computer
    $L.Add((C '  COMPUTER' '1;97'))
    $memF = $cache.MemUsed / [Math]::Max(0.1, $cache.MemTot)
    $L.Add(('  {0,-12}' -f 'Processor') + (Bar ($cache.Cpu / 100) 20 '36') + ('  {0,3}%' -f $cache.Cpu) + ('        {0,-8}' -f 'Memory') + (Bar $memF 20 '35') + ('  {0:N1} of {1:N1} GB' -f $cache.MemUsed, $cache.MemTot))
    $diskF = 1 - $cache.DiskFree / [Math]::Max(1, $cache.DiskTot)
    $L.Add(('  {0,-12}' -f 'Disk D:') + (Bar $diskF 20 '33') + ('  {0:N0} GB free' -f $cache.DiskFree) + ('   results so far {0:N2} GB' -f $cache.Size))
    if ($null -ne $cache.Bat) {
        $pw = if ($cache.AC) { C ("plugged in, battery $($cache.Bat)%") '32' } else { C ("ON BATTERY ($($cache.Bat)%): please plug in, runs are much slower") '1;31' }
        $L.Add(('  {0,-12}' -f 'Power') + $pw)
    } else { $L.Add('') }
    $L.Add('')

    # log
    $L.Add((C '  LAST STEPS' '1;97'))
    foreach ($ln in $cache.Log) {
        if ($ln.Length -ge 21) {
            $t = (UtcToLocal $ln.Substring(0, 20)).ToString('hh:mm tt'); $txt = $ln.Substring(21)
            if ($txt.Length -gt $W - 14) { $txt = $txt.Substring(0, $W - 17) + '...' }
            $col = if ($txt -match 'FAILED') { '31' } elseif ($txt -match 'nRF|PASS|done|tree in') { '32' } else { '37' }
            $L.Add('  ' + (C $t '90') + '  ' + (C $txt $col))
        }
    }
    $L.Add((C ('─' * $W) '90'))
    $L.Add((C '  Updates live. Press Q to close this window; the benchmark keeps running.' '90'))

    if ($Once) { $L | ForEach-Object { $_ }; break }
    # draw on the program's own screen: every line at its fixed row, cut to the
    # window width (no wrapping), never more rows than the window has (no scrolling)
    $Hn = $Host.UI.RawUI.WindowSize.Height; if (-not $Hn -or $Hn -lt 8) { $Hn = 40 }
    $Wn = $Host.UI.RawUI.WindowSize.Width; if (-not $Wn -or $Wn -lt 20) { $Wn = 100 }
    $rows = $Hn - 1
    if ($L.Count -gt $rows) { $keep = @($L[0..($rows - 2)]) + @($L[$L.Count - 1]) } else { $keep = @($L) }
    $out = New-Object System.Text.StringBuilder
    for ($i = 0; $i -lt $keep.Count; $i++) {
        [void]$out.Append("$esc[$($i + 1);1H" + (Fit $keep[$i] ($Wn - 1)) + "$esc[K")
    }
    for ($i = $keep.Count; $i -lt $Hn; $i++) { [void]$out.Append("$esc[$($i + 1);1H$esc[K") }
    [void]$out.Append("$esc[1;1H")
    [Console]::Write($out.ToString())
    for ($k = 0; $k -lt 10; $k++) {
        Start-Sleep -Milliseconds 100
        try { if ([Console]::KeyAvailable) { $key = [Console]::ReadKey($true); if ($key.Key -eq 'Q') { Leave; exit } } } catch {}
    }
}
