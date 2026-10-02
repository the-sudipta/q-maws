#!/bin/sh
# Comparison of wQFM-rs with the original wQFM jar (development only; needs
# Java, git and network access). Checks out wQFM at a fixed commit outside
# the repository, writes 23 weighted-quartet inputs with `qmaws wqfm-export`
# (the worksheet example, 10 noise-free and 10 noisy simulated inputs, and
# the W2c quartets of Fish mtDNA with and without the strand filter), runs
# the jar on each and compares with `qmaws wqfm-compare`.
#
# Usage: sh scripts/wqfm_check.sh [work folder]   (default: ../_external)
# Exit status 0 when every input meets the criteria of plan 2.8. On GitHub
# Actions the results are also written as annotations.

set -u

COMMIT=7bfdf8e77bd9079c708c831f1fb8404cc6821809
root=$(git rev-parse --show-toplevel) || exit 1
work=${1:-"$root/../_external"}
mkdir -p "$work"
work=$(cd "$work" && pwd)
qmaws="$root/target/release/qmaws"
[ -x "$qmaws" ] || qmaws="$qmaws.exe"

annotate() { # level title message
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        echo "::$1 title=$2::$3"
    fi
    echo "$2: $3"
}

# 1. wQFM at the inspected commit (the jar and its lib folder are in the
#    repository; Apache License 2.0).
if [ ! -d "$work/wQFM-2020/.git" ]; then
    git clone -q https://github.com/Mahim1997/wQFM-2020 "$work/wQFM-2020" || exit 1
fi
(cd "$work/wQFM-2020" && git checkout -q "$COMMIT") || exit 1
jar="$work/wQFM-2020/wQFM-v1.4.jar"
[ -f "$jar" ] || { annotate error "wQFM" "wQFM-v1.4.jar not found"; exit 1; }
annotate notice "Java" "$(java -version 2>&1 | head -n 1)"

# 2. Inputs, including two Fish mtDNA runs.
runs="$work/wqfm_runs"
out="$work/wqfm_inputs"
rm -rf "$runs" "$out"
"$qmaws" --quiet run --dataset fish_mito --output "$runs/fish_strand" || exit 1
"$qmaws" --quiet run --dataset fish_mito --no-strand --output "$runs/fish_no_strand" || exit 1
"$qmaws" wqfm-export --output "$out" --run "$runs/fish_strand" --run "$runs/fish_no_strand" --seed 1 || exit 1

# 3. The jar on every input (default settings: partition score [s] - [v]).
tab=$(printf '\t')
tail -n +2 "$out/inputs.tsv" | while IFS="$tab" read -r name rest; do
    (cd "$work/wQFM-2020" && java -Xmx6000M -jar "$jar" -i "$out/$name.wqrts" -o "$out/$name.jar.tre" > "$out/$name.jar.log" 2>&1) \
        || { annotate error "wQFM jar" "$name failed: $(tail -n 3 "$out/$name.jar.log" | tr '\n' ' ')"; exit 1; }
done || exit 1

# 4. Comparison.
"$qmaws" wqfm-compare --dir "$out" > "$work/wqfm_comparison.txt"
status=$?
cat "$work/wqfm_comparison.txt"
summary=$(tail -n 1 "$work/wqfm_comparison.txt")
if [ $status -eq 0 ]; then
    annotate notice "wQFM comparison" "$summary"
else
    annotate error "wQFM comparison" "$summary"
fi
# One annotation per kind of input (GitHub shows at most 10 notices per
# step): name, score ratio, nRF between the two trees, result.
tail -n +2 "$out/comparison.tsv" | awk -F'\t' '{k=$2; a[k]=a[k] " | " $1 ": ratio " $8 ", nRF " $9 ", " $12} END {for (k in a) print k a[k]}' | sort |
    while read -r line; do
        annotate notice "wQFM rows" "$line"
    done
exit $status
