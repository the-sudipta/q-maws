#!/bin/sh
# ML-MAWS runs for hypothesis H4 (development only; needs Linux x86-64, g++
# with OpenMP, git, curl and network access). Builds ML-MAWS at the
# inspected commit and IQ-TREE 2.4.0 (the program ML-MAWS calls as
# `iqtree2`) outside the repository, and runs ML-MAWS as documented
# (`--strand --iqtree`: IQ-TREE -st BIN -m MFP+ASC -bb 1000) on one
# simulated HGT dataset. Support values do not depend on the computer; the
# versions, settings and IQ-TREE's seed (from its log) are recorded
# (docs/PREREGISTRATION.md, Amendment 3).
#
# Usage: sh scripts/mlmaws_h4.sh <dataset id, e.g. sim_hgt_0> <output folder> [work folder]
# Writes to <output folder>: ML_MAWS_tree.newick (the tree with UFBoot
# support), the IQ-TREE log and report, ML-MAWS's complexity report and
# messages, and provenance.txt.

set -u

COMMIT=0c38db12d9ad271aafcb4940d7558dfcd00925c1
IQ_VERSION=2.4.0
IQ_URL="https://github.com/iqtree/iqtree2/releases/download/v$IQ_VERSION/iqtree-$IQ_VERSION-Linux-intel.tar.gz"
# GitHub lists no digest for this release; the SHA-256 is recorded on first
# use (provenance.txt) and pinned here afterwards. Empty: record only.
IQ_SHA256=""

dataset=${1:?dataset id}
out=${2:?output folder}
root=$(git rev-parse --show-toplevel) || exit 1
work=${3:-"$root/../_external"}
mkdir -p "$work" "$out"
work=$(cd "$work" && pwd)
out=$(cd "$out" && pwd)
qmaws="$root/target/release/qmaws"

annotate() { # level title message
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        echo "::$1 title=$2::$3"
    fi
    echo "$2: $3"
}

# 1. ML-MAWS at the inspected commit (as in scripts/equivalence.sh).
if [ ! -d "$work/ML-MAWS/.git" ]; then
    git clone -q https://github.com/PapriSaha/ML-MAWS "$work/ML-MAWS" || exit 1
fi
(cd "$work/ML-MAWS" && git checkout -q "$COMMIT") || exit 1
ml="$work/ML-MAWS/ml-maws"
if [ ! -x "$ml" ]; then
    (cd "$work/ML-MAWS" && g++ -O2 -std=c++17 -fopenmp -include cstdint -o ml-maws ./*.cpp) \
        > "$work/ml-maws-build.txt" 2>&1 || { annotate error "ML-MAWS build" "g++ failed"; exit 1; }
fi

# 2. IQ-TREE 2.4.0, put on PATH as iqtree2.
archive="$work/iqtree-$IQ_VERSION.tar.gz"
[ -f "$archive" ] || curl -sSfL -o "$archive" "$IQ_URL" || { annotate error "IQ-TREE download" "$IQ_URL failed"; exit 1; }
iq_sha=$(sha256sum "$archive" | cut -d' ' -f1)
if [ -n "$IQ_SHA256" ] && [ "$iq_sha" != "$IQ_SHA256" ]; then
    annotate error "IQ-TREE download" "SHA-256 $iq_sha differs from the pinned $IQ_SHA256"
    exit 1
fi
tar -xzf "$archive" -C "$work" || exit 1
iq=$(find "$work" -type f -name iqtree2 | head -n 1)
[ -n "$iq" ] || { annotate error "IQ-TREE" "no iqtree2 binary in the archive"; exit 1; }
chmod +x "$iq"
PATH="$(dirname "$iq"):$PATH"
export PATH

# 3. The dataset, downloaded and checked by qmaws (MD5 of the archive).
"$qmaws" --quiet download --dataset "$dataset" --data-dir "$root/data" || exit 1
case "$dataset" in
    sim_hgt_*) input="$root/data/raw/sim_hgt/simulated-sim_hgt/hgt_${dataset#sim_hgt_}" ;;
    *) annotate error "$dataset" "H4 uses the simulated HGT datasets only"; exit 1 ;;
esac
[ -d "$input" ] || { annotate error "$dataset" "input folder $input not found"; exit 1; }

# 4. ML-MAWS as documented, with the runner's cores for IQ-TREE.
threads=$(nproc)
run="$work/run_$dataset"
rm -rf "$run"
mkdir -p "$run"
start=$(date +%s)
"$ml" -i "$input" -o "$run" --strand --iqtree --threads "$threads" > "$run/stdout.txt" 2> "$run/stderr.txt"
code=$?
seconds=$(( $(date +%s) - start ))
for f in ML_MAWS_tree.newick ml_maws_iqtree.log ml_maws_iqtree.iqtree ml_maws_iqtree.contree complexity_report.json entropy_results.tsv stdout.txt stderr.txt; do
    [ -f "$run/$f" ] && cp "$run/$f" "$out/"
done
seed=$(grep -m 1 -o 'Seed: *[0-9]*' "$run/ml_maws_iqtree.log" 2>/dev/null | grep -o '[0-9]*$')
{
    echo "dataset: $dataset"
    echo "input: $input"
    echo "ml_maws_commit: $COMMIT"
    echo "ml_maws_command: ml-maws -i <input> -o <out> --strand --iqtree --threads $threads"
    echo "ml_maws_build: g++ -O2 -std=c++17 -fopenmp -include cstdint ($(g++ --version | head -n 1))"
    echo "iqtree: $("$iq" --version 2>/dev/null | grep -m 1 -i 'version')"
    echo "iqtree_archive: $IQ_URL"
    echo "iqtree_archive_sha256: $iq_sha"
    echo "iqtree_seed: ${seed:-not found}"
    echo "exit_status: $code"
    echo "wall_seconds: $seconds (GitHub runner; not used for H2)"
    echo "threads: $threads"
    echo "cpu: $(grep -m 1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ *//')"
    echo "date_utc: $(date -u +%FT%TZ)"
} > "$out/provenance.txt"
if [ $code -ne 0 ] || [ ! -f "$out/ML_MAWS_tree.newick" ]; then
    annotate error "$dataset" "ML-MAWS failed (exit $code); see stderr.txt"
    exit 1
fi
annotate notice "$dataset" "ML-MAWS tree with UFBoot support written; IQ-TREE seed ${seed:-?}; $seconds s; archive SHA-256 $iq_sha"
