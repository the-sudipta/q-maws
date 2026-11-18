#!/bin/sh
# Makes the simulated datasets again on a Windows runner (Amendment 10): AliSim gives
# other data on Linux, so the programs of the laptop are used, IQ-TREE 2.4.0 for
# Windows and Q-MAWS 1.0.0 for Windows, with the committed scripts and seeds:
#   scripts/simulate_benchmark.sh   the 36 datasets of Amendment 8
#   scripts/simulate_scaling.sh     the 9 speed datasets of M13
# Every dataset is compared with the committed manifests (SHA-256) and the result is
# written to hash_check.tsv; a dataset that differs is not run (Amendment 10).
#
# Usage (Git Bash on windows-2025, from the repository folder):
#   sh scripts/cloud/inputs_windows.sh <work folder> <output folder>
# Writes <output>/benchmark.tar, <output>/scaling.tar and <output>/hash_check.tsv.

set -eu

work=${1:?work folder}
out=${2:?output folder}
root=$(git rev-parse --show-toplevel)
cd "$root"
mkdir -p "$work" "$out"

QMAWS_URL=https://github.com/the-sudipta/q-maws/releases/download/v1.0.0/Q-MAWS-1.0.0-windows-x64.zip
QMAWS_SHA256=1b4dc0a24127def3779ea74828558881d28297c92f6d5b93f50bb6c4d087e9ea
IQ_URL=https://github.com/iqtree/iqtree2/releases/download/v2.4.0/iqtree-2.4.0-Windows.zip
IQ_SHA256=93f488a6cfef96ef664b23fefe5be97c2c80ebcae9cd2031c0cac92b467278be
man=data/manifests/simulated

fetch() { # url file sha256
    curl -sSfL --retry 5 -o "$2" "$1"
    echo "$3  $2" | sha256sum -c --quiet -
}

# 1. the programs
fetch "$QMAWS_URL" "$work/qmaws.zip" "$QMAWS_SHA256"
fetch "$IQ_URL" "$work/iqtree.zip" "$IQ_SHA256"
7z x -y -o"$work/qmaws" "$work/qmaws.zip" > /dev/null    # 7-Zip is on every Windows runner
7z x -y -o"$work" "$work/iqtree.zip" > /dev/null
qmaws=$(find "$work/qmaws" -name Q-MAWS.exe | head -n 1)
iq="$work/iqtree-2.4.0-Windows/bin/iqtree2.exe"
"$qmaws" --version
"$iq" --version | head -n 3

# 2. the H5 datasets with h = 0.2, which the fragmented datasets are cut from (branch h5-data)
git fetch -q --depth 1 origin h5-data
for r in 1 2 3 4 5; do
    id=hgt_h020_r$r
    mkdir -p "data/raw/simulated/$id"
    git show "FETCH_HEAD:h5data/$id.fasta.gz" | gzip -dc > "data/raw/simulated/$id/sequences.fasta"
    want=$(awk -F'\t' -v d="$id" '$1 == d { print $8 }' "$man/hgt.tsv")
    echo "$want  data/raw/simulated/$id/sequences.fasta" | sha256sum -c --quiet -
done

# 3. the simulations; the scripts write the manifests again, which are compared below
cp "$man/benchmark.tsv" "$work/benchmark.committed.tsv"
cp "$man/scaling.tsv" "$work/scaling.committed.tsv"
sh scripts/simulate_benchmark.sh "$iq" "$qmaws" python
sh scripts/simulate_scaling.sh "$iq"

# 4. hash check: dataset, expected SHA-256, SHA-256 made here, equal
compare() { # committed table, new table, column of the sequences hash
    awk -F'\t' -v c="$3" 'NR == FNR { if (FNR > 1) want[$1] = $c; next }
        FNR > 1 && ($1 in want) { printf "%s\t%s\t%s\t%s\n", $1, want[$1], $c, (want[$1] == $c ? "yes" : "no") }' "$1" "$2"
}
{
    printf 'dataset\texpected_sha256\tmade_sha256\tequal\n'
    compare "$work/benchmark.committed.tsv" "$man/benchmark.tsv" 11
    compare "$work/scaling.committed.tsv" "$man/scaling.tsv" 7
} > "$out/hash_check.tsv"
# the species trees and transfer tables must be equal too (git compares them byte for byte)
git diff --stat -- "$man" > "$out/manifest_diff.txt" || true
cp "$work/benchmark.committed.tsv" "$man/benchmark.tsv"
cp "$work/scaling.committed.tsv" "$man/scaling.tsv"
git checkout -- "$man"
cat "$out/hash_check.tsv"
echo "datasets equal to the manifests: $(grep -c 'yes$' "$out/hash_check.tsv"), different: $(grep -c 'no$' "$out/hash_check.tsv")"

# 5. pack (the 200-taxa dataset is left out, Amendment 7)
tar -cf "$out/benchmark.tar" data/raw/simulated/bm_*
tar -cf "$out/scaling.tar" data/raw/simulated/scaling_m25_* data/raw/simulated/scaling_m50_* data/raw/simulated/scaling_m100_*
ls -l "$out"
