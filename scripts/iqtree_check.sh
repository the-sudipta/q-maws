#!/bin/sh
# IQ-TREE cross-check of the conditioned quartet likelihood (development
# only; needs Linux x86-64, curl and network access). Downloads the official
# IQ-TREE release outside the repository, checks its SHA-256, exports 5
# seeded quartets of Fish mtDNA with `qmaws iqtree-export`, fits every
# topology in IQ-TREE under the matching two-state model with ascertainment
# correction, and compares with `qmaws iqtree-compare` (tolerance 0.0001).
#
# Usage: sh scripts/iqtree_check.sh [work folder]   (default: ../_external)
# Exit status 0 when every log-likelihood matches. On GitHub Actions the
# summary and the comparison rows are also written as annotations.

set -u

VERSION=3.1.4
URL="https://github.com/iqtree/iqtree3/releases/download/v$VERSION/iqtree-$VERSION-Linux-intel.tar.gz"
SHA256=d422cb2b8f04825faea753afda25c60de6611537d07ec8fcd0033e70cb042839

root=$(git rev-parse --show-toplevel) || exit 1
work=${1:-"$root/../_external"}
mkdir -p "$work"
work=$(cd "$work" && pwd)
qmaws="$root/target/release/qmaws"

annotate() { # level title message
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        echo "::$1 title=$2::$3"
    fi
    echo "$2: $3"
}

# 1. IQ-TREE, checked against the digest GitHub lists for the release asset.
archive="$work/iqtree-$VERSION.tar.gz"
if [ ! -f "$archive" ]; then
    curl -sSfL -o "$archive" "$URL" || { annotate error "IQ-TREE download" "$URL failed"; exit 1; }
fi
echo "$SHA256  $archive" | sha256sum -c - || { annotate error "IQ-TREE download" "SHA-256 differs"; exit 1; }
tar -xzf "$archive" -C "$work" || exit 1
iq=$(find "$work" -type f -name iqtree3 | head -n 1)
[ -n "$iq" ] || { annotate error "IQ-TREE" "no iqtree3 binary in the archive"; exit 1; }
chmod +x "$iq"
annotate notice "IQ-TREE version" "$("$iq" --version | grep -m 1 -i 'version')"

# 2. Quartets of Fish mtDNA (counting only; the weighting stage is not needed).
run="$work/iqtree_run"
out="$work/iqtree_quartets"
rm -rf "$run" "$out"
"$qmaws" --quiet run --dataset fish_mito --weighting none --output "$run" || exit 1
"$qmaws" iqtree-export --run "$run" --output "$out" --quartets 5 --seed 1 || exit 1

# 3. IQ-TREE: each quartet, model and topology with Q-MAWS's branch lengths
#    fixed (-blfix), with branch lengths optimised (-te fixes the topology),
#    and optimised starting from Q-MAWS's lengths.
cd "$out" || exit 1
tab=$(printf '\t')
tail -n +2 expected.tsv | while IFS="$tab" read -r q model t iqmodel rest; do
    p="q${q}_${model}_t${t}"
    common="-s q$q.phy -st BIN -m $iqmodel -blmin 0.000001 -blmax 10 -nt 1 -seed 1 -redo -quiet"
    # shellcheck disable=SC2086
    "$iq" $common -te "${p}_fixed.nwk" -blfix -pre "${p}_fixed" >/dev/null 2>&1 \
        || { annotate error "IQ-TREE" "$p fixed lengths failed: $(tail -n 3 "${p}_fixed.log" | tr '\n' ' ')"; exit 1; }
    # shellcheck disable=SC2086
    "$iq" $common -te "q${q}_t$t.nwk" -me 0.000001 -pre "${p}_opt" >/dev/null 2>&1 \
        || { annotate error "IQ-TREE" "$p optimisation failed: $(tail -n 3 "${p}_opt.log" | tr '\n' ' ')"; exit 1; }
    # shellcheck disable=SC2086
    "$iq" $common -te "${p}_fixed.nwk" -me 0.000001 -pre "${p}_warm" >/dev/null 2>&1 \
        || { annotate error "IQ-TREE" "$p optimisation from Q-MAWS lengths failed: $(tail -n 3 "${p}_warm.log" | tr '\n' ' ')"; exit 1; }
done || exit 1
cd "$root" || exit 1

# 4. Comparison.
"$qmaws" iqtree-compare --run "$run" --dir "$out" > "$work/iqtree_comparison.txt"
status=$?
cat "$work/iqtree_comparison.txt"
summary=$(tail -n 1 "$work/iqtree_comparison.txt")
if [ $status -eq 0 ]; then
    annotate notice "IQ-TREE cross-check" "$summary"
else
    annotate error "IQ-TREE cross-check" "$summary"
fi
# One annotation per quartet with its six rows, so the numbers can be read
# without the job log (GitHub shows at most 10 notices per step).
tail -n +2 "$out/comparison.tsv" | awk -F'\t' '{k="quartet " $1; a[k]=a[k] " | " $2 " t" $3 ": max " $4 ", A " $6 ", max diff " $8 ", B " $10 ", C " $12} END {for (k in a) print k a[k]}' | sort |
    while read -r line; do
        annotate notice "IQ-TREE rows" "$line"
    done
exit $status
