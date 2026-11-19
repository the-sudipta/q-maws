#!/bin/sh
# The analysis of the extended benchmark (Amendments 8 and 9; descriptive analyses and
# H6), once every run is finished. It runs the committed analysis scripts unchanged on
# the runs of the state branch and writes their tables back into the state.
#
# Usage (from the repository folder, with target/release/qmaws = Q-MAWS 1.0.0):
#   sh scripts/cloud/analyse.sh <state checkout>

set -eu

state=${1:?state checkout}
root=$(git rev-parse --show-toplevel)
cd "$root"

# the analysis scripts read results/benchmark/ of the repository folder
rm -rf results/benchmark
cp -r "$state/results/benchmark" results/benchmark

python3 scripts/bench_hash.py verify          # every sealed run is unchanged
python3 scripts/bench_collect.py              # summary/accuracy.tsv
python3 scripts/bench_stats.py --check        # H6, medians, Friedman, pairwise (after reproducing H5)
python3 scripts/bench_extra.py                # calibration, halo, wrong splits
python3 scripts/bench_dependence.py \
    fish_mito=results/benchmark/dependence/fish_mito \
    hgt_h000_r1=results/benchmark/dependence/hgt_h000_r1 \
    hgt_h010_r1=results/benchmark/dependence/hgt_h010_r1 \
    hgt_h020_r1=results/benchmark/dependence/hgt_h020_r1 \
    hgt_h040_r1=results/benchmark/dependence/hgt_h040_r1
python3 scripts/bench_speed.py                # speed runs, compared per processor model
python3 scripts/bench_w2c1000.py              # W2c with 1,000 against 100 resamples

rm -rf "$state/results/benchmark/summary"
cp -r results/benchmark/summary "$state/results/benchmark/summary"
echo "analysis written to the state: results/benchmark/summary/"
