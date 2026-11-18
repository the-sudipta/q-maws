#!/bin/sh
# Collects every input of the benchmark on a Linux runner and packs it in one archive
# per group (Amendment 10):
#   afproject  the 9 AFproject datasets              (qmaws download, published checksums)
#   ncbi       the 5 NCBI datasets of M11            (for the checks of the reused runs)
#   h5         the 20 H5 datasets                    (branch h5-data, checked against hgt.tsv),
#              with their gene partitions made again by `qmaws hgt-trees` (checked)
#   benchmark  the 36 datasets of Amendment 8        (branch benchmark-data, Amendment 10a)
#   scaling    the 9 speed datasets of M13           (branch benchmark-data, Amendment 10a)
# The datasets of the branch benchmark-data were made on the development laptop by
# make_simulated_data.sh; each is used only if it equals the committed manifests
# (check_simulated.sh); hash_check.tsv records the check.
#
# Usage (from the repository folder):
#   sh scripts/cloud/inputs_linux.sh <qmaws program> <output folder>
# Writes <output>/input-<group>.tar, <output>/inputs.json and <output>/hash_check.tsv.

set -eu

qmaws=${1:?qmaws program}
out=${2:?output folder}
root=$(git rev-parse --show-toplevel)
cd "$root"
mkdir -p "$out"
man=data/manifests/simulated
H5_IDS=$(awk -F'\t' 'NR > 1 { print $1 }' "$man/hgt.tsv")

# 1. AFproject and NCBI data
"$qmaws" --quiet download --dataset all --data-dir data

# 2. H5: sequences from the branch h5-data, then the gene partitions of their recipe
git fetch -q --depth 1 origin h5-data
for id in $H5_IDS; do
    dir=data/raw/simulated/$id
    mkdir -p "$dir"
    git show "FETCH_HEAD:h5data/$id.fasta.gz" | gzip -dc > "$dir/sequences.fasta"
    echo "$(awk -F'\t' -v d="$id" '$1 == d { print $8 }' "$man/hgt.tsv")  $dir/sequences.fasta" | sha256sum -c --quiet -
    # the species tree is the committed copy of AliSim's species.treefile (checked by its hash)
    cp "$man/trees/$id.nwk" "$dir/species.treefile"
    echo "$(awk -F'\t' -v d="$id" '$1 == d { print $9 }' "$man/hgt.tsv")  $dir/species.treefile" | sha256sum -c --quiet -
    genes=$(awk -F'\t' -v d="$id" '$1 == d { print $6 }' "$man/hgt.tsv")
    (cd "$dir" && eval "\"$qmaws\" --quiet ${genes#qmaws }" > genes.log)
    echo "$(awk -F'\t' -v d="$id" '$1 == d { print $10 }' "$man/hgt.tsv")  $dir/genes/transfers.tsv" | sha256sum -c --quiet -
done

# 3. the simulated data made on the development laptop (one archive per dataset), checked
git fetch -q --depth 1 origin benchmark-data
for f in $(git ls-tree --name-only FETCH_HEAD benchdata/ | grep '\.tar\.gz$'); do
    git show "FETCH_HEAD:$f" | tar -xzf -
done
sh scripts/cloud/check_simulated.sh "$out/hash_check.tsv"

# 4. sizes and taxon counts (used to plan the jobs), and the archives
python3 scripts/cloud/bench_cloud.py inputs --data . --out "$out/inputs.json"
tar -cf "$out/input-afproject.tar" data/raw/fish_mito data/raw/ecoli data/raw/ecoli_shigella_hgt data/raw/yersinia_hgt data/raw/sim_hgt
tar -cf "$out/input-ncbi.tar" data/raw/coronavirus data/raw/ebolavirus data/raw/influenza_a data/raw/mammal_mtdna data/raw/rhinovirus
tar -cf "$out/input-h5.tar" data/raw/simulated/hgt_*
tar -cf "$out/input-benchmark.tar" data/raw/simulated/bm_*
tar -cf "$out/input-scaling.tar" data/raw/simulated/scaling_*
ls -l "$out"
