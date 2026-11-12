#!/bin/bash
# Alignment-based references of the extended benchmark (docs/PREREGISTRATION.md, Amendment 9),
# run inside Ubuntu (WSL) on the 45 simulated datasets whose true alignment is known:
#   fasttree_genes  one FastTree 2 tree per gene of the true alignment (genes/partitions.nex)
#   caster_site     CASTER-site on the true genome alignment
#   treeqmc         TREE-QMC on the gene trees (no weights)
#   wtreeqmc        weighted TREE-QMC on the gene trees (hybrid weights; FastTree's local
#                   supports lie in [0, 1], so the likelihood-support preset -L is used)
# Restartable: a run folder with done.txt or failed.txt is skipped. Wall time and peak memory
# go to results/benchmark/runs.tsv like every other run. It can run beside phase B: its
# scratch folders (ref_*) never match the ones phase B removes (bench_*).
#
# Usage (inside WSL, as root): bash scripts/bench_refs.sh

set -u
root=/mnt/d/q-maws
cd "$root" || exit 1
B=/opt/bench
T=12
OUT=results/benchmark
RUNS="$OUT/runs.tsv"
LOG="$OUT/queue_refs.log"
SCRATCH=$root/.benchtmp
mkdir -p "$SCRATCH"
say() { echo "$(date -u +%FT%TZ) $*" | tee -a "$LOG"; }

DATASETS="hgt_h000_r1 hgt_h000_r2 hgt_h000_r3 hgt_h000_r4 hgt_h000_r5 hgt_h010_r1 hgt_h010_r2 hgt_h010_r3 hgt_h010_r4 hgt_h010_r5
hgt_h020_r1 hgt_h020_r2 hgt_h020_r3 hgt_h020_r4 hgt_h020_r5 hgt_h040_r1 hgt_h040_r2 hgt_h040_r3 hgt_h040_r4 hgt_h040_r5
bm_div005_r1 bm_div005_r2 bm_div005_r3 bm_div005_r4 bm_div005_r5 bm_div020_r1 bm_div020_r2 bm_div020_r3 bm_div020_r4 bm_div020_r5
bm_div050_r1 bm_div050_r2 bm_div050_r3 bm_div050_r4 bm_div050_r5 bm_m15_r1 bm_m15_r2 bm_m15_r3 bm_m15_r4 bm_m15_r5
bm_m60_r1 bm_m60_r2 bm_m60_r3 bm_m60_r4 bm_m60_r5"

# one gene alignment per charset of partitions.nex: gene_<n>.fa in folder $3
split_genes() { # sequences.fasta partitions.nex folder
    mkdir -p "$3"
    awk -v dir="$3" '
        FNR == NR { if ($1 == "charset") { split($4, r, "-"); sub(";", "", r[2]); n++; lo[n] = r[1]; hi[n] = r[2] } next }
        /^>/ { name = $0; next }
        { seq[name] = seq[name] $0 }
        END { for (g = 1; g <= n; g++) { f = sprintf("%s/gene_%04d.fa", dir, g)
                for (s in seq) print s "\n" substr(seq[s], lo[g], hi[g] - lo[g] + 1) > f
                close(f) } }' "$2" "$1"
}

# run one step with GNU time: step <dataset> <method> <folder> <command...>
step() {
    ds=$1; m=$2; dir=$3; shift 3
    { [ -f "$dir/done.txt" ] || [ -f "$dir/failed.txt" ]; } && return 0
    rm -rf "$dir"; mkdir -p "$dir"
    say "$m on $ds"
    s0=$(date -u +%FT%TZ)
    /usr/bin/time -v -o "$dir/time.txt" bash -c "$*" > "$dir/run.log" 2>&1
    rc=$?
    s1=$(date -u +%FT%TZ)
    secs=$(awk -F': ' '/Elapsed \(wall clock\)/{n=split($2,a,":"); s=0; for(i=1;i<=n;i++) s=s*60+a[i]; printf "%d", s}' "$dir/time.txt")
    kb=$(awk -F': ' '/Maximum resident set size/{print $2}' "$dir/time.txt")
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$ds" "$m" "$s0" "$s1" "${secs:-NA}" "$(( ${kb:-0} / 1024 ))" "$rc" >> "$RUNS"
    if [ "$rc" -eq 0 ] && [ -s "$dir/tree.nwk" ]; then
        echo "tree=tree.nwk" > "$dir/done.txt"; say "$m on $ds: done"
    else
        echo "exit $rc; see run.log" > "$dir/failed.txt"; say "$m on $ds: FAILED (exit $rc)"
    fi
}

say "references started"
for ds in $DATASETS; do
    data="data/raw/simulated/$ds"
    g="$OUT/$ds/fasttree_genes"
    w="$SCRATCH/ref_$ds"
    if [ ! -f "$g/done.txt" ] && [ ! -f "$g/failed.txt" ]; then
        rm -rf "$w"; split_genes "$data/sequences.fasta" "$data/genes/partitions.nex" "$w"
        echo "FastTree $(FastTree 2>&1 | head -1 | sed 's/^Usage for //')" > "$w/version.txt"
    fi
    # FastTree is single-threaded: 12 genes at a time; tree.nwk = all gene trees, one per line
    step "$ds" fasttree_genes "$g" \
        "ls '$w'/gene_*.fa | xargs -P $T -I{} sh -c 'FastTree -nt -gtr -quiet {} > {}.tre 2>/dev/null' &&
         for f in '$w'/gene_*.fa.tre; do cat \"\$f\"; done > '$g/tree.nwk' &&
         [ \$(wc -l < '$g/tree.nwk') -eq \$(ls '$w'/gene_*.fa | wc -l) ] && cp '$w/version.txt' '$g/version.txt' &&
         echo 'FastTree -nt -gtr -quiet gene_<n>.fa (one tree per gene of genes/partitions.nex)' > '$g/command.txt'"
    rm -rf "$w"
    step "$ds" caster_site "$OUT/$ds/caster_site" \
        "cd '$OUT/$ds/caster_site' && $B/ASTER/bin/caster-site -t $T -o tree.nwk '$root/$data/sequences.fasta' &&
         echo 'caster-site -t $T -o tree.nwk sequences.fasta' > command.txt &&
         echo \"ASTER commit \$(cd $B/ASTER && git rev-parse --short HEAD), caster-site \$($B/ASTER/bin/caster-site -h 2>&1 | grep -m1 Version)\" > version.txt"
    if [ -f "$g/done.txt" ]; then
        step "$ds" treeqmc "$OUT/$ds/treeqmc" \
            "cd '$OUT/$ds/treeqmc' && $B/TREE-QMC/build/tree-qmc -i '$root/$g/tree.nwk' -o tree.nwk &&
             echo 'tree-qmc -i gene_trees -o tree.nwk' > command.txt && echo 'TREE-QMC v3.0.4 (b5bfe82)' > version.txt"
        step "$ds" wtreeqmc "$OUT/$ds/wtreeqmc" \
            "cd '$OUT/$ds/wtreeqmc' && $B/TREE-QMC/build/tree-qmc --hybrid -L -i '$root/$g/tree.nwk' -o tree.nwk &&
             echo 'tree-qmc --hybrid -L -i gene_trees -o tree.nwk' > command.txt && echo 'TREE-QMC v3.0.4 (b5bfe82)' > version.txt"
    fi
done
say "references finished"
