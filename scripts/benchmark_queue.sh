#!/bin/sh
# Restartable job queue of the extended benchmark (docs/PREREGISTRATION.md,
# Amendments 8 and 8a). Start it with scripts/start_benchmark.bat (double
# click) or `sh scripts/benchmark_queue.sh` from the repository folder.
#
# Every job writes its result folder and a "done" marker. A finished job is
# skipped, so the queue can be stopped at any time (close the window) and
# started again later; it continues with the first unfinished job. An
# interrupted Q-MAWS run is continued with `qmaws resume`; an interrupted
# ML-MAWS or IQ-TREE run is started again from its beginning.
#
# Phase A (Amendment 8a, Windows-native methods):
#   1. qmaws verify --quick (release 1.0.0) on every reused run
#   2. on each of the 30 new evaluation datasets: Q-MAWS, ML-MAWS, and the
#      IQ-TREE reference on the true alignment (not for fragmented data)
#   3. ML-MAWS on the 9 AFproject datasets
#   4. the IQ-TREE reference on the 20 H5 datasets
# Phase B (Amendment 8b): the other methods are added here once fixed.
#
# Every run records wall time and peak memory in results/benchmark/runs.tsv.

set -u
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 1
export PATH=/usr/bin:/mingw64/bin:$PATH

QMAWS="$root/target/release/qmaws.exe"
ML=/d/_external/ML-MAWS/ml-maws.exe
IQ=/d/_external/tools/iqtree-2.4.0-Windows/bin/iqtree2.exe
export PATH="$PATH:/d/_external/tools/w64devkit/bin:/d/_external/tools/iqtree-2.4.0-Windows/bin"
THREADS=12
OUT=results/benchmark
LOG="$OUT/queue.log"
RUNS="$OUT/runs.tsv"
mkdir -p "$OUT"
[ -f "$RUNS" ] || printf 'dataset\tmethod\tstart_utc\tend_utc\tseconds\tpeak_mb\texit\n' > "$RUNS"

say() { echo "$(date -u +%FT%TZ) $*" | tee -a "$LOG"; }

# --- one queue at a time --------------------------------------------------
LOCK="$OUT/queue.lock"
mywin=$(cat /proc/$$/winpid 2>/dev/null || echo 0)
if [ -f "$LOCK" ]; then
    other=$(cat "$LOCK")
    if powershell -NoProfile -Command "if (Get-Process -Id $other -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }"; then
        echo "The benchmark queue is already running (process $other). Nothing to do."
        exit 0
    fi
fi
echo "$mywin" > "$LOCK"
cleanup() { rm -f "$LOCK" "$OUT"/.stop_*; }
trap cleanup EXIT INT TERM HUP

# --- checks ---------------------------------------------------------------
for f in "$QMAWS" "$ML" "$IQ"; do
    [ -x "$f" ] || { say "missing program: $f"; exit 1; }
done
"$QMAWS" --version | grep -q "1.0.0" || { say "Q-MAWS is not release 1.0.0: $("$QMAWS" --version)"; exit 1; }
ac=$(powershell -NoProfile -Command "(Get-CimInstance Win32_Battery).BatteryStatus" 2>/dev/null | tr -d '\r')
[ "$ac" = "2" ] || [ -z "$ac" ] || echo "NOTE: the laptop seems to be on battery. Runs are much slower on battery; please plug it in."

# --- timing and memory of one run ------------------------------------------
# timed <dataset> <method> <process names, comma separated> <command...>
timed() {
    ds=$1; method=$2; names=$3; shift 3
    stop="$OUT/.stop_$$"
    peakf="$OUT/.peak_$$"
    rm -f "$stop" "$peakf"
    powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$root/scripts/mem_peak.ps1")" \
        -Names $names -Out "$(cygpath -w "$peakf")" -Stop "$(cygpath -w "$stop")" -Owner "$mywin" &
    t0=$(date -u +%s); s0=$(date -u +%FT%TZ)
    "$@"
    rc=$?
    t1=$(date -u +%s); s1=$(date -u +%FT%TZ)
    touch "$stop"; wait
    peak=$(cat "$peakf" 2>/dev/null | tr -d '\r'); rm -f "$stop" "$peakf"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$ds" "$method" "$s0" "$s1" $((t1 - t0)) "${peak:-NA}" "$rc" >> "$RUNS"
    return $rc
}

# --- the job list -----------------------------------------------------------
NEW="bm_div005_r1 bm_div005_r2 bm_div005_r3 bm_div005_r4 bm_div005_r5
bm_div020_r1 bm_div020_r2 bm_div020_r3 bm_div020_r4 bm_div020_r5
bm_div050_r1 bm_div050_r2 bm_div050_r3 bm_div050_r4 bm_div050_r5
bm_m15_r1 bm_m15_r2 bm_m15_r3 bm_m15_r4 bm_m15_r5
bm_m60_r1 bm_m60_r2 bm_m60_r3 bm_m60_r4 bm_m60_r5
bm_frag_r1 bm_frag_r2 bm_frag_r3 bm_frag_r4 bm_frag_r5"
H5="hgt_h000_r1 hgt_h000_r2 hgt_h000_r3 hgt_h000_r4 hgt_h000_r5
hgt_h010_r1 hgt_h010_r2 hgt_h010_r3 hgt_h010_r4 hgt_h010_r5
hgt_h020_r1 hgt_h020_r2 hgt_h020_r3 hgt_h020_r4 hgt_h020_r5
hgt_h040_r1 hgt_h040_r2 hgt_h040_r3 hgt_h040_r4 hgt_h040_r5"
AF="fish_mito:fish_mito/assembled-fish_mito ecoli:ecoli/assembled-ecoli
ecoli_shigella_hgt:ecoli_shigella_hgt/unsimulated-ecoli_shigella
yersinia_hgt:yersinia_hgt/unsimulated-yersinia
sim_hgt_0:sim_hgt/simulated-sim_hgt/hgt_0 sim_hgt_250:sim_hgt/simulated-sim_hgt/hgt_250
sim_hgt_500:sim_hgt/simulated-sim_hgt/hgt_500 sim_hgt_750:sim_hgt/simulated-sim_hgt/hgt_750
sim_hgt_1000:sim_hgt/simulated-sim_hgt/hgt_1000"

input_of() { # the input of a new dataset: one multi-FASTA, or a folder of files
    case $1 in
        bm_frag_*) echo "data/raw/simulated/$1" ;;
        *) echo "data/raw/simulated/$1/sequences.fasta" ;;
    esac
}

reused_runs() {
    for d in results/runs/*/ results/h5/*/qmaws/ results/scaling/*/qmaws/; do
        [ -f "$d/run.json" ] && echo "${d%/}"
    done
}

# --- progress ---------------------------------------------------------------
count_jobs() {
    total=0; done_=0
    for r in $(reused_runs); do
        total=$((total + 1)); n=$(echo "$r" | tr '/' '_')
        [ -f "$OUT/verify/$n.txt" ] && done_=$((done_ + 1))
    done
    for id in $NEW; do
        total=$((total + 2)); [ -f "$OUT/$id/qmaws/audit/root.txt" ] && done_=$((done_ + 1))
        [ -f "$OUT/$id/ml_maws/done.txt" ] && done_=$((done_ + 1))
        case $id in bm_frag_*) ;; *) total=$((total + 1)); [ -f "$OUT/$id/iqtree_true/done.txt" ] && done_=$((done_ + 1)) ;; esac
    done
    for pair in $AF; do
        id=${pair%%:*}; total=$((total + 1)); [ -f "$OUT/$id/ml_maws/done.txt" ] && done_=$((done_ + 1))
    done
    for id in $H5; do
        total=$((total + 1)); [ -f "$OUT/$id/iqtree_true/done.txt" ] && done_=$((done_ + 1))
    done
    echo "$done_ of $total jobs done"
}

# --- jobs ---------------------------------------------------------------
job_verify() { # reused run folder
    n=$(echo "$1" | tr '/' '_')
    [ -f "$OUT/verify/$n.txt" ] && return 0
    mkdir -p "$OUT/verify"
    say "verify $1"
    "$QMAWS" --no-color verify --quick --seed 1 --output "$1" > "$OUT/verify/$n.part" 2>&1
    mv "$OUT/verify/$n.part" "$OUT/verify/$n.txt"
    say "verify $1: $(grep -o 'Verdict: [A-Z]*' "$OUT/verify/$n.txt" | head -1)"
}

job_qmaws() { # dataset
    id=$1; dir="$OUT/$id/qmaws"
    [ -f "$dir/audit/root.txt" ] && return 0
    mkdir -p "$OUT/$id"
    if [ -f "$dir/run.json" ]; then
        say "Q-MAWS $id (continue)"
        timed "$id" qmaws qmaws "$QMAWS" --quiet resume --output "$dir" > "$OUT/$id/qmaws.out.txt" 2>&1
    else
        say "Q-MAWS $id"
        timed "$id" qmaws qmaws "$QMAWS" --quiet run --input "$(input_of "$id")" \
            --reference "data/manifests/simulated/trees/$id.nwk" --seed 1 --bootstrap 0 \
            --no-live-tree --output "$dir" > "$OUT/$id/qmaws.out.txt" 2>&1
    fi
    say "Q-MAWS $id: exit $?"
}

ml_once() { # input folder-or-file, output folder: ML-MAWS, then (above 50 taxa) its normal-mode IQ-TREE
    "$ML" -i "$1" -o "$2" --strand --iqtree --threads "$THREADS" > "$2/stdout.txt" 2> "$2/stderr.txt"
    if [ ! -s "$2/ML_MAWS_tree.newick" ] && [ -f "$2/ml_maws_matrix.phy" ]; then
        (cd "$2" && "$IQ" -s ml_maws_matrix.phy -st BIN -m MFP+ASC -bb 1000 -nt "$THREADS" \
            --prefix ml_maws_iqtree -redo > iqtree_normal_mode.txt 2>&1)
    fi
    [ -s "$2/ML_MAWS_tree.newick" ] || [ -s "$2/ml_maws_iqtree.treefile" ]
}

job_mlmaws() { # dataset input
    id=$1; dir="$OUT/$id/ml_maws"
    [ -f "$dir/done.txt" ] && return 0
    rm -rf "$dir"; mkdir -p "$dir"
    say "ML-MAWS $id"
    if timed "$id" ml_maws ml-maws,iqtree2 ml_once "$2" "$dir"; then
        tree=ML_MAWS_tree.newick; [ -s "$dir/$tree" ] || tree=ml_maws_iqtree.treefile
        echo "tree=$tree" > "$dir/done.txt"
        say "ML-MAWS $id: tree in $tree"
    else
        say "ML-MAWS $id: FAILED (see $dir/stderr.txt); it will be tried again next time"
    fi
    rm -f "$dir/ml_maws_matrix.phy" "$dir/ml_maws_matrix.tsv" "$dir/ml_maws_iqtree.ckp.gz" \
          "$dir/ml_maws_iqtree.mldist" "$dir/ml_maws_iqtree.model.gz" "$dir/ml_maws_iqtree.bionj" \
          "$dir/ml_maws_iqtree.parstree" "$dir/ml_maws_iqtree.splits.nex"
}

job_iqtree() { # dataset (alignment = data/raw/simulated/<id>/sequences.fasta)
    id=$1; dir="$OUT/$id/iqtree_true"
    [ -f "$dir/done.txt" ] && return 0
    rm -rf "$dir"; mkdir -p "$dir"
    aln="$root/data/raw/simulated/$id/sequences.fasta"
    say "IQ-TREE reference $id"
    if timed "$id" iqtree_true iqtree2 sh -c "cd '$dir' && '$IQ' -s '$aln' -m MFP -nt $THREADS -seed 1 --prefix true_aln -redo > iqtree.out.txt 2>&1"; then
        echo "tree=true_aln.treefile" > "$dir/done.txt"
        rm -f "$dir/true_aln.ckp.gz" "$dir/true_aln.mldist" "$dir/true_aln.model.gz" "$dir/true_aln.bionj" "$dir/true_aln.uniqueseq.phy"
        say "IQ-TREE reference $id: done"
    else
        say "IQ-TREE reference $id: FAILED; it will be tried again next time"
    fi
}

# --- run ----------------------------------------------------------------
say "queue started: $(count_jobs)"
for r in $(reused_runs); do job_verify "$r"; done
for id in $NEW; do
    job_qmaws "$id"
    job_mlmaws "$id" "$(input_of "$id")"
    case $id in bm_frag_*) ;; *) job_iqtree "$id" ;; esac
    say "progress: $(count_jobs)"
done
for pair in $AF; do
    job_mlmaws "${pair%%:*}" "data/raw/${pair#*:}"
done
for id in $H5; do job_iqtree "$id"; done

status=$(count_jobs)
say "queue stopped: $status"
finished=$(echo "$status" | awk '{print $1}')
total=$(echo "$status" | awk '{print $3}')
if [ "$finished" = "$total" ]; then
    echo; echo "=========================================="
    echo "  PHASE A COMPLETED: every job is done."
    echo "=========================================="
    say "PHASE A COMPLETED"
else
    echo "Some jobs failed. Start the queue again; it retries only the unfinished jobs."
fi
