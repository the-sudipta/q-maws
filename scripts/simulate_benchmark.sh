#!/bin/sh
# New simulated datasets of the extended benchmark (docs/PREREGISTRATION.md,
# Amendment 8; committed before any of these data were made). Every dataset
# follows the H5 design (scripts/simulate_hgt.sh): an AliSim Yule-Harding
# species tree, 1,000 genes of 1,000 sites, a fraction h of the genes with one
# random SPR of the species tree (`qmaws hgt-trees`), AliSim topology-unlinked
# partitions, Jukes-Cantor. Branch lengths -rlen min mean max with
# min = mean / 10 and max = mean x 10 (H5: 0.001 0.01 0.1).
#
#   divergence  bm_div<mean>_r<rep>: h = 0.2, 30 taxa, mean 0.005, 0.02, 0.05;
#               seed 9000 + 100 x index + rep (index 0, 1, 2)
#   size        bm_m<taxa>_r<rep>:   h = 0.2, mean 0.01, 15 or 60 taxa;
#               seed 9300 + 100 x index + rep (index 0, 1)
#   tuning      bm_tune_r<rep>:      h = 0.2, 30 taxa, mean 0.01, 6 replicates;
#               seed 9600 + rep (never used for evaluation)
#   fragmented  bm_frag_r<rep>:      the H5 datasets hgt_h020_r<rep> cut into
#               contigs by scripts/fragment_genomes.py, seed 9700 + rep
# The genome seed is the dataset seed + 50000, as in H5.
#
# Usage: sh scripts/simulate_benchmark.sh <path to iqtree2> <path to qmaws> <python>
# Writes data/raw/simulated/<id>/ (not committed) and, committed:
# data/manifests/simulated/benchmark.tsv (commands, seeds, SHA-256), the
# species trees in data/manifests/simulated/trees/ and the transfer tables in
# data/manifests/simulated/transfers/.

set -eu

iq=${1:?path to iqtree2}
qmaws=${2:?path to qmaws}
py=${3:?path to python}
root=$(git rev-parse --show-toplevel)
raw="$root/data/raw/simulated"
man="$root/data/manifests/simulated"
mkdir -p "$raw" "$man/trees" "$man/transfers"
table="$man/benchmark.tsv"
printf 'dataset\tcondition\ttaxa\tfraction\tmean_branch\treplicate\tseed\tspecies_command\tgenes_command\tgenome_command\tsequences_sha256\tspecies_tree_sha256\ttransfers_sha256\n' > "$table.tmp"

one() { # id condition taxa fraction min mean max replicate seed
    id=$1; cond=$2; m=$3; h=$4; lo=$5; mu=$6; hi=$7; r=$8; seed=$9
    gseed=$((seed + 50000))
    dir="$raw/$id"
    rm -rf "$dir"
    mkdir -p "$dir"
    c1="iqtree2 --alisim species -t RANDOM{yh/$m} -rlen $lo $mu $hi -m JC --length 10 -seed $seed"
    c2="qmaws hgt-trees --species species.treefile --genes 1000 --gene-length 1000 --fraction $h --seed $seed --output genes"
    c3="iqtree2 --alisim genome -Q genes/partitions.nex -t genes/gene_trees.nwk --out-format fasta -seed $gseed"
    (cd "$dir" && "$iq" --alisim species -t "RANDOM{yh/$m}" -rlen "$lo" "$mu" "$hi" -m JC --length 10 -seed "$seed" > species.log 2>&1)
    (cd "$dir" && "$qmaws" hgt-trees --species species.treefile --genes 1000 --gene-length 1000 --fraction "$h" --seed "$seed" --output genes > genes.log 2>&1)
    (cd "$dir" && "$iq" --alisim genome -Q genes/partitions.nex -t genes/gene_trees.nwk --out-format fasta -seed "$gseed" > genome.log 2>&1)
    sed 's/[[:space:]]*$//' "$dir/genome.fa" > "$dir/sequences.fasta"
    rm "$dir/genome.fa" "$dir/species.phy"
    cp "$dir/species.treefile" "$man/trees/$id.nwk"
    cp "$dir/genes/transfers.tsv" "$man/transfers/$id.tsv"
    fa=$(sha256sum "$dir/sequences.fasta" | cut -d' ' -f1)
    tr=$(sha256sum "$man/trees/$id.nwk" | cut -d' ' -f1)
    tx=$(sha256sum "$man/transfers/$id.tsv" | cut -d' ' -f1)
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$cond" "$m" "$h" "$mu" "$r" "$seed" "$c1" "$c2" "$c3" "$fa" "$tr" "$tx" >> "$table.tmp"
    echo "$id: seed $seed, $(tail -n +2 "$man/transfers/$id.tsv" | wc -l) transferred genes, sequences SHA-256 $fa"
}

# divergence
i=0
for spec in "005 0.0005 0.005 0.05" "020 0.002 0.02 0.2" "050 0.005 0.05 0.5"; do
    set -- $spec
    for r in 1 2 3 4 5; do
        one "bm_div$1_r$r" divergence 30 0.2 "$2" "$3" "$4" "$r" $((9000 + 100 * i + r))
    done
    i=$((i + 1))
done
# number of species
i=0
for m in 15 60; do
    for r in 1 2 3 4 5; do
        one "bm_m${m}_r$r" size "$m" 0.2 0.001 0.01 0.1 "$r" $((9300 + 100 * i + r))
    done
    i=$((i + 1))
done
# tuning (never evaluated)
for r in 1 2 3 4 5 6; do
    one "bm_tune_r$r" tuning 30 0.2 0.001 0.01 0.1 "$r" $((9600 + r))
done
# fragmented assemblies of the H5 datasets with h = 0.2
for r in 1 2 3 4 5; do
    src="$raw/hgt_h020_r$r/sequences.fasta"
    dir="$raw/bm_frag_r$r"
    rm -rf "$dir"
    "$py" "$root/scripts/fragment_genomes.py" "$src" "$dir" $((9700 + r))
    cp "$man/trees/hgt_h020_r$r.nwk" "$man/trees/bm_frag_r$r.nwk"
    fa=$(cat "$dir"/*.fasta | sha256sum | cut -d' ' -f1)
    tr=$(sha256sum "$man/trees/bm_frag_r$r.nwk" | cut -d' ' -f1)
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "bm_frag_r$r" fragmented 30 0.2 0.01 "$r" $((9700 + r)) "from hgt_h020_r$r" "-" "python scripts/fragment_genomes.py sequences.fasta bm_frag_r$r $((9700 + r))" "$fa" "$tr" "-" >> "$table.tmp"
    echo "bm_frag_r$r: contigs of hgt_h020_r$r, seed $((9700 + r)), files SHA-256 $fa"
done
mv "$table.tmp" "$table"
echo "Manifest: $table"
