#!/bin/sh
# The analysis of the extended benchmark (Amendments 8 and 9; descriptive analyses and
# H6), once every run is finished. It runs the committed analysis scripts unchanged on
# the runs of the state branch and writes their tables back into the state.
#
# Every script runs even when an earlier one stops (Amendment 10c): a script that stops
# is named, with its exit status, in summary/analysis_errors.txt, which is committed with
# the tables, so one fault never hides the other analyses. The workflow marks the job
# as failed after committing when that file is not empty.
#
# Usage (from the repository folder, with target/release/qmaws = Q-MAWS 1.0.0):
#   sh scripts/cloud/analyse.sh <state checkout>

set -u

state=${1:?state checkout}
root=$(git rev-parse --show-toplevel)
cd "$root" || exit 1

# the analysis scripts read results/benchmark/ of the repository folder
rm -rf results/benchmark
cp -r "$state/results/benchmark" results/benchmark || exit 1
mkdir -p results/benchmark/summary
errors=results/benchmark/summary/analysis_errors.txt
: > "$errors"

step() {
    name=$1; shift
    echo "== $name"
    "$@"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "$name: exit $rc" >> "$errors"
        echo "::error title=Analysis::$name stopped with exit $rc"
    fi
}

step bench_hash       python3 scripts/bench_hash.py verify          # every sealed run is unchanged
step bench_collect    python3 scripts/bench_collect.py              # summary/accuracy.tsv
step bench_stats      python3 scripts/bench_stats.py --check        # H6, medians, Friedman, pairwise (after reproducing H5)
step bench_extra      python3 scripts/bench_extra.py                # calibration, halo, wrong splits
step bench_dependence python3 scripts/bench_dependence.py \
    fish_mito=results/benchmark/dependence/fish_mito \
    hgt_h000_r1=results/benchmark/dependence/hgt_h000_r1 \
    hgt_h010_r1=results/benchmark/dependence/hgt_h010_r1 \
    hgt_h020_r1=results/benchmark/dependence/hgt_h020_r1 \
    hgt_h040_r1=results/benchmark/dependence/hgt_h040_r1
step bench_speed      python3 scripts/bench_speed.py                # speed runs, compared per processor model
step bench_w2c1000    python3 scripts/bench_w2c1000.py              # W2c with 1,000 against 100 resamples

rm -rf "$state/results/benchmark/summary"
cp -r results/benchmark/summary "$state/results/benchmark/summary" || exit 1
if [ -s "$errors" ]; then
    echo "analysis written to the state with errors:"; cat "$errors"
else
    rm -f "$state/results/benchmark/summary/analysis_errors.txt"
    echo "analysis written to the state: results/benchmark/summary/"
fi
