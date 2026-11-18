#!/bin/sh
# Makes the simulated datasets again with the programs that made them (Amendments 10 and
# 10a): IQ-TREE 2.4.0 for Windows (AliSim) and Q-MAWS 1.0.0 for Windows, on the
# development laptop, with the committed scripts and seeds:
#   scripts/simulate_benchmark.sh   the 36 datasets of Amendment 8
#   scripts/simulate_scaling.sh     the 9 speed datasets of M13
# AliSim gives other data on Linux and on Windows runners with other processors
# (Amendment 10a), so these data are made here and stored on the branch benchmark-data.
# Every dataset is compared with the committed manifests (SHA-256) and the result is
# written to hash_check.tsv; the archives are made only if every dataset is equal.
#
# Usage (Git Bash, from the repository folder; PYTHON = a Python 3, default python):
#   sh scripts/cloud/make_simulated_data.sh <work folder> <output folder>
# Writes <output>/hash_check.tsv and <output>/benchdata/<dataset>.tar.gz (one per dataset).

set -eu

work=${1:?work folder}
out=${2:?output folder}
python=${PYTHON:-python}      # a Python 3 for scripts/fragment_genomes.py
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
unzip_to() { # zip folder: 7-Zip (on every Windows runner), or PowerShell
    if command -v 7z > /dev/null; then
        7z x -y -o"$2" "$1" > /dev/null
    else
        powershell -NoProfile -Command "Expand-Archive -Force -LiteralPath '$(cygpath -w "$1")' -DestinationPath '$(cygpath -w "$2")'"
    fi
}
unzip_to "$work/qmaws.zip" "$work/qmaws"
unzip_to "$work/iqtree.zip" "$work"
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
sh scripts/simulate_benchmark.sh "$iq" "$qmaws" "$python"
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

[ "$(grep -c 'no$' "$out/hash_check.tsv")" -eq 0 ] || { echo "some datasets differ: nothing is packed"; exit 1; }

# 5. one archive per dataset for the branch benchmark-data (200 taxa left out, Amendment 7);
#    the AliSim logs are left out (they hold the computer's name and times, not data)
mkdir -p "$out/benchdata"
for dir in data/raw/simulated/bm_* data/raw/simulated/scaling_m25_* data/raw/simulated/scaling_m50_* data/raw/simulated/scaling_m100_*; do
    tar --force-local --exclude='*.log' -czf "$out/benchdata/$(basename "$dir").tar.gz" "$dir"
done
ls -l "$out/benchdata"
