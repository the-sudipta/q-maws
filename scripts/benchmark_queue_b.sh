#!/bin/sh
# Phase B of the extended benchmark (docs/PREREGISTRATION.md, Amendment 8b):
# the Linux methods, run in Ubuntu (WSL) through scripts/bench_tools.sh.
#   1. tuning: every value of each tuning grid on the 6 tuning datasets;
#      then scripts/bench_tune_choose.py picks the tuned values
#   2. evaluation: every method on the 59 evaluation datasets with its
#      default setting, and again with the tuned value where it differs
#   3. speed: every method on the 9 speed datasets (run alone; please do not
#      use the laptop meanwhile)
# Restartable like phase A: a job with done.txt or failed.txt is skipped.
# Every tree is compared with its reference by `qmaws tree-compare`; wall
# time and peak memory (GNU time, largest process) go to results/benchmark/runs.tsv.
# Finished runs are sealed with SHA-256 (scripts/bench_hash.py) after tuning,
# after every evaluation dataset and at the end.

set -u
root=$(cd "${0%/*}/.." && pwd)
cd "$root" || exit 1
export PATH=/usr/bin:/mingw64/bin:$PATH
export MSYS_NO_PATHCONV=1
PS="/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
QMAWS="$root/target/release/qmaws.exe"
DISTRO=Ubuntu-24.04
OUT=results/benchmark
LOG="$OUT/queue.log"
RUNS="$OUT/runs.tsv"
mkdir -p "$OUT"
[ -f "$RUNS" ] || printf 'dataset\tmethod\tstart_utc\tend_utc\tseconds\tpeak_mb\texit\n' > "$RUNS"
say() { echo "$(date -u +%FT%TZ) $*" | tee -a "$LOG"; }

# --- one queue at a time (shared lock with phase A) ---------------------------
LOCK="$OUT/queue.lock"
mywin=$(cat /proc/$$/winpid 2>/dev/null || echo 0)
if [ -f "$LOCK" ]; then
    other=$(cat "$LOCK")
    if "$PS" -NoProfile -Command "\$p = Get-Process -Id $other -ErrorAction SilentlyContinue; if (\$p -and (\$p.Name -in 'sh','bash')) { exit 0 } else { exit 1 }"; then
        echo "The benchmark queue is already running (process $other). Nothing to do."
        exit 0
    fi
fi
echo "$mywin" > "$LOCK"
trap 'rm -f "$LOCK"' EXIT INT TERM HUP

"$QMAWS" --version | grep -q "1.0.0" || { say "Q-MAWS is not release 1.0.0"; exit 1; }
# leftovers of an interrupted run inside Ubuntu are stopped first
wsl.exe --terminate "$DISTRO" > /dev/null 2>&1

TOOLS="mash cdmaws peafowl andi phylonium fswm multispam sans cvtree jolytree skmer mashtree"
grid_of() {
    case $1 in
        mash|mashtree) echo "15 17 19 21 23 25 31" ;;
        sans|skmer) echo "15 19 23 27 31" ;;
        fswm) echo "8 10 12 14 16" ;;
        multispam) echo "8 10 12" ;;
        cvtree) echo "7 9 11 13" ;;
        *) echo "" ;;
    esac
}
TUNE="bm_tune_r1 bm_tune_r2 bm_tune_r3 bm_tune_r4 bm_tune_r5 bm_tune_r6"
H5="hgt_h000_r1 hgt_h000_r2 hgt_h000_r3 hgt_h000_r4 hgt_h000_r5 hgt_h010_r1 hgt_h010_r2 hgt_h010_r3 hgt_h010_r4 hgt_h010_r5
hgt_h020_r1 hgt_h020_r2 hgt_h020_r3 hgt_h020_r4 hgt_h020_r5 hgt_h040_r1 hgt_h040_r2 hgt_h040_r3 hgt_h040_r4 hgt_h040_r5"
NEW="bm_div005_r1 bm_div005_r2 bm_div005_r3 bm_div005_r4 bm_div005_r5 bm_div020_r1 bm_div020_r2 bm_div020_r3 bm_div020_r4 bm_div020_r5
bm_div050_r1 bm_div050_r2 bm_div050_r3 bm_div050_r4 bm_div050_r5 bm_m15_r1 bm_m15_r2 bm_m15_r3 bm_m15_r4 bm_m15_r5
bm_m60_r1 bm_m60_r2 bm_m60_r3 bm_m60_r4 bm_m60_r5 bm_frag_r1 bm_frag_r2 bm_frag_r3 bm_frag_r4 bm_frag_r5"
AF="fish_mito ecoli ecoli_shigella_hgt yersinia_hgt sim_hgt_0 sim_hgt_250 sim_hgt_500 sim_hgt_750 sim_hgt_1000"
SPEED="scaling_m25_r1 scaling_m25_r2 scaling_m25_r3 scaling_m50_r1 scaling_m50_r2 scaling_m50_r3 scaling_m100_r1 scaling_m100_r2 scaling_m100_r3"

input_of() {
    case $1 in
        fish_mito) echo data/raw/fish_mito/assembled-fish_mito ;;
        ecoli) echo data/raw/ecoli/assembled-ecoli ;;
        ecoli_shigella_hgt) echo data/raw/ecoli_shigella_hgt/unsimulated-ecoli_shigella ;;
        yersinia_hgt) echo data/raw/yersinia_hgt/unsimulated-yersinia ;;
        sim_hgt_*) echo "data/raw/sim_hgt/simulated-sim_hgt/hgt_${1#sim_hgt_}" ;;
        bm_frag_*) echo "data/raw/simulated/$1" ;;
        *) echo "data/raw/simulated/$1/sequences.fasta" ;;
    esac
}
ref_of() {
    case $1 in
        sim_hgt_*) echo data/references/sim_hgt.nwk ;;
        fish_mito|ecoli|ecoli_shigella_hgt|yersinia_hgt) echo "data/references/$1.nwk" ;;
        *) echo "data/manifests/simulated/trees/$1.nwk" ;;
    esac
}

# one method on one dataset: run_job <dataset> <method> <out folder> [setting]
run_job() {
    ds=$1; m=$2; dir=$3; set_=${4:-}
    { [ -f "$dir/done.txt" ] || [ -f "$dir/failed.txt" ]; } && return 0
    rm -rf "$dir"; mkdir -p "$dir"
    say "$m${set_:+ ($set_)} on $ds"
    s0=$(date -u +%FT%TZ)
    wsl.exe -d "$DISTRO" -u root -- /usr/bin/time -v -o "/mnt/d/q-maws/$dir/time.txt" \
        bash /mnt/d/q-maws/scripts/bench_tools.sh "$m" "/mnt/d/q-maws/$(input_of "$ds")" "/mnt/d/q-maws/$dir" $set_ \
        > "$dir/run.log" 2>&1
    rc=$?
    s1=$(date -u +%FT%TZ)
    secs=$(tr -d '\r' < "$dir/time.txt" 2>/dev/null | awk -F': ' '/Elapsed \(wall clock\)/{n=split($2,a,":"); s=0; for(i=1;i<=n;i++) s=s*60+a[i]; printf "%d", s}')
    kb=$(tr -d '\r' < "$dir/time.txt" 2>/dev/null | awk -F': ' '/Maximum resident set size/{print $2}')
    mb=NA; [ -n "$kb" ] && mb=$((kb / 1024))
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$ds" "$m${set_:+_$set_}" "$s0" "$s1" "${secs:-NA}" "$mb" "$rc" >> "$RUNS"
    if [ -s "$dir/tree.nwk" ]; then
        "$QMAWS" --quiet tree-compare --tree "$dir/tree.nwk" --reference "$(ref_of "$ds")" > "$dir/comparison.tsv" 2> "$dir/comparison_error.txt"
        echo "ok" > "$dir/done.txt"
        say "$m on $ds: $(grep -m1 '^nRF' "$dir/comparison.tsv" | cut -f2 | sed 's/^/nRF /')"
    else
        echo "exit $rc; see run.log" > "$dir/failed.txt"
        say "$m on $ds: FAILED (exit $rc)"
    fi
}

all_done() { # folder pattern list: returns 0 when every listed job has a marker
    for d in "$@"; do { [ -f "$d/done.txt" ] || [ -f "$d/failed.txt" ]; } || return 1; done; return 0
}

say "phase B started"
# 1. tuning
tune_dirs=""
for m in $TOOLS; do
    for v in $(grid_of "$m"); do
        for ds in $TUNE; do
            d="$OUT/tuning/${m}_$v/$ds"; tune_dirs="$tune_dirs $d"
            run_job "$ds" "$m" "$d" "$v"
        done
    done
done
if all_done $tune_dirs; then
    python scripts/bench_tune_choose.py "$OUT/tuning" "$OUT/tuning/chosen.tsv" > /dev/null
    python scripts/bench_hash.py seal > /dev/null
    say "tuning done: $(tail -n +2 "$OUT/tuning/chosen.tsv" | cut -f1,3 | tr '\t\n' '= ' )"
else
    say "tuning incomplete; start the queue again"; exit 1
fi
chosen_of() { awk -F'\t' -v m="$1" '$1==m{print $3}' "$OUT/tuning/chosen.tsv"; }
default_of() { awk -F'\t' -v m="$1" '$1==m{print $2}' "$OUT/tuning/chosen.tsv"; }

# 2. evaluation
for ds in $AF $H5 $NEW; do
    for m in $TOOLS; do
        run_job "$ds" "$m" "$OUT/$ds/$m"
        c=$(chosen_of "$m"); dflt=$(default_of "$m")
        [ -n "$c" ] && [ "$c" != "$dflt" ] && run_job "$ds" "$m" "$OUT/$ds/${m}_tuned" "$c"
    done
    python scripts/bench_hash.py seal > /dev/null
    say "evaluation: $ds finished"
done

# 3. speed (run alone)
echo; echo "Speed runs: please do not use the laptop until they finish (it keeps the timings fair)."
for ds in $SPEED; do
    for m in $TOOLS; do run_job "$ds" "$m" "$OUT/speed/$ds/$m"; done
done
python scripts/bench_hash.py seal | tail -1 | sed "s/^/  /"
say "PHASE B COMPLETED"
echo; echo "=========================================="
echo "  PHASE B COMPLETED: the benchmark runs are done."
echo "=========================================="
