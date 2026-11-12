#!/bin/sh
# W2c with 1,000 resamples instead of 100 (docs/PREREGISTRATION.md, Amendment 9; descriptive):
# Q-MAWS 1.0.0 on the five H5 datasets with h = 0.2, seed 1, no S2 bootstrap.
# Restartable: a finished run (audit/root.txt) is skipped, an unfinished one is resumed.
# Usage (Git Bash, from the repository folder): sh scripts/bench_w2c1000.sh
cd "$(dirname "$0")/.." || exit 1
Q=target/release/qmaws.exe
for r in 1 2 3 4 5; do
    id=hgt_h020_r$r; dir=results/benchmark/w2c1000/$id
    [ -f "$dir/audit/root.txt" ] && continue
    mkdir -p results/benchmark/w2c1000
    if [ -f "$dir/run.json" ]; then
        "$Q" --quiet resume --output "$dir"
    else
        "$Q" --quiet run --input "data/raw/simulated/$id/sequences.fasta" \
            --reference "data/manifests/simulated/trees/$id.nwk" --seed 1 --bootstrap 0 \
            --replicates 1000 --cores 6 --no-live-tree --output "$dir"
    fi
    echo "$(date -u +%FT%TZ) $id exit $?"
done
