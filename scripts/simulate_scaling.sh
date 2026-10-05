#!/bin/sh
# Simulated scaling datasets for hypothesis H2 (plan 6.5; docs/PREREGISTRATION.md
# Amendment 3): genomes of 1,000,000 letters on random Yule-Harding trees with
# m = 25, 50 and 100 taxa (3 replicates each) and m = 200 (1 replicate),
# simulated with AliSim (IQ-TREE 2.4.0) under its defaults (Jukes-Cantor,
# branch lengths exponential with mean 0.1, between 0.001 and 0.999), with a
# fixed seed per dataset: seed = 1000 * m + replicate.
#
# Usage: sh scripts/simulate_scaling.sh <path to iqtree2 executable>
# Writes data/raw/simulated/scaling_m<m>_r<rep>/sequences.fasta (not
# committed) and data/manifests/simulated/scaling.tsv plus the true trees in
# data/manifests/simulated/trees/ (committed). Trailing spaces in AliSim's
# FASTA headers are removed; the SHA-256 is of the file after that step.
# Running it again gives the same files (checked against the manifest).

set -eu

iq=${1:?path to iqtree2}
root=$(git rev-parse --show-toplevel)
raw="$root/data/raw/simulated"
man="$root/data/manifests/simulated"
mkdir -p "$raw" "$man/trees"
table="$man/scaling.tsv"
printf 'dataset\ttaxa\treplicate\tseed\tlength\tcommand\tfasta_sha256\ttree_sha256\n' > "$table.tmp"

one() { # taxa replicate
    m=$1; r=$2
    id="scaling_m${m}_r${r}"
    seed=$((1000 * m + r))
    dir="$raw/$id"
    mkdir -p "$dir"
    cmd="iqtree2 --alisim alignment -t RANDOM{yh/$m} -m JC --length 1000000 --out-format fasta -seed $seed"
    (cd "$dir" && "$iq" --alisim alignment -t "RANDOM{yh/$m}" -m JC --length 1000000 \
        --out-format fasta -seed "$seed" > alisim.log 2>&1)
    sed 's/[[:space:]]*$//' "$dir/alignment.fa" > "$dir/sequences.fasta"
    rm "$dir/alignment.fa"
    cp "$dir/alignment.treefile" "$man/trees/$id.nwk"
    fa=$(sha256sum "$dir/sequences.fasta" | cut -d' ' -f1)
    tr=$(sha256sum "$man/trees/$id.nwk" | cut -d' ' -f1)
    printf '%s\t%s\t%s\t%s\t1000000\t%s\t%s\t%s\n' "$id" "$m" "$r" "$seed" "$cmd" "$fa" "$tr" >> "$table.tmp"
    echo "$id: seed $seed, sequences SHA-256 $fa"
}

for m in 25 50 100; do
    for r in 1 2 3; do
        one "$m" "$r"
    done
done
one 200 1
mv "$table.tmp" "$table"
echo "Manifest: $table"
