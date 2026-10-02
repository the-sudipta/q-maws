#!/bin/sh
# Equivalence test with ML-MAWS (development only; needs g++ with OpenMP,
# git and network access). Builds ML-MAWS at a fixed commit outside the
# repository, runs it and `qmaws matrix` on the same datasets, and compares
# the PHYLIP matrices byte for byte and the entropy tables value by value.
#
# Usage: sh scripts/equivalence.sh [work folder]   (default: ../_external)
# Exit status 0 when every dataset is identical. On GitHub Actions the result
# of each dataset is also written as a notice or error annotation.

set -u

COMMIT=0c38db12d9ad271aafcb4940d7558dfcd00925c1
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

# 1. ML-MAWS at the inspected commit.
if [ ! -d "$work/ML-MAWS/.git" ]; then
    git clone -q https://github.com/PapriSaha/ML-MAWS "$work/ML-MAWS" || exit 1
fi
(cd "$work/ML-MAWS" && git checkout -q "$COMMIT") || exit 1
ml="$work/ML-MAWS/ml-maws"
if [ ! -x "$ml" ]; then
    # The repository has no Makefile; compile all sources directly.
    if ! (cd "$work/ML-MAWS" && g++ -O2 -std=c++17 -fopenmp -o ml-maws ./*.cpp) >"$work/ml-maws-build.txt" 2>&1; then
        # The first compiler messages, on one line (annotations are one line).
        first=$(grep -E 'error|Error' "$work/ml-maws-build.txt" | head -5 | tr '\n' ' ' | cut -c1-900)
        annotate error "ML-MAWS build" "g++ failed: $first"
        exit 1
    fi
fi

failed=0
check() { # dataset id, input folder
    id=$1
    input=$2
    out_ml="$work/equivalence/$id/ml-maws"
    out_q="$work/equivalence/$id/qmaws"
    rm -rf "$out_ml" "$out_q"
    mkdir -p "$out_ml"
    "$ml" -i "$input" -o "$out_ml" --strand --no-raxml >"$out_ml/stdout.txt" 2>"$out_ml/stderr.txt" || {
        annotate error "$id" "ML-MAWS failed (see $out_ml/stderr.txt)"
        failed=1
        return
    }
    "$qmaws" --quiet matrix --dataset "$id" --data-dir "$root/data" --output "$out_q" >"$work/equivalence/$id/qmaws.txt" 2>&1 || {
        annotate error "$id" "qmaws matrix failed"
        failed=1
        return
    }
    # Entropy tables: lengths and column counts exactly; entropies to the
    # 6 significant digits ML-MAWS prints.
    entropy=$(awk -F'\t' '
        NR == FNR { if (FNR > 1) { c[$1] = $2; e[$1] = $3 } next }
        FNR > 1 {
            n++
            if (!($1 in c)) { bad = bad " length " $1 " missing"; next }
            if (c[$1] != $2) bad = bad " length " $1 ": " c[$1] " vs " $2 " columns"
            d = e[$1] - $3; if (d < 0) d = -d
            m = $3 < 0 ? -$3 : $3
            if (d > 5e-6 * (m > 1 ? m : 1)) bad = bad " length " $1 ": entropy " e[$1] " vs " $3
        }
        END { if (bad == "") print "same (" n " lengths)"; else print "DIFFERENT:" bad }
    ' "$out_ml/entropy_results.tsv" "$out_q/entropy.tsv")
    lengths_ml=$(grep -o 'selected lengths = {[^}]*}' "$out_ml/stderr.txt" | head -1)
    lengths_q=$(grep -o '"selected_lengths": \[[^]]*\]' "$out_q/summary.json" | tr -d '\n ')
    if cmp -s "$out_ml/ml_maws_matrix.phy" "$out_q/m_ml.phy"; then
        dims=$(head -1 "$out_q/m_ml.phy")
        annotate notice "$id" "M_ml identical to ML-MAWS (PHYLIP $dims, byte for byte); entropy tables $entropy; ML-MAWS $lengths_ml, Q-MAWS $lengths_q"
    else
        annotate error "$id" "M_ml differs from ML-MAWS: ML-MAWS $(head -1 "$out_ml/ml_maws_matrix.phy"), Q-MAWS $(head -1 "$out_q/m_ml.phy"); entropy tables $entropy; ML-MAWS $lengths_ml, Q-MAWS $lengths_q"
        failed=1
    fi
}

check fish_mito "$root/data/raw/fish_mito/assembled-fish_mito"
check yersinia_hgt "$root/data/raw/yersinia_hgt/unsimulated-yersinia"
exit $failed
