#!/bin/sh
# Simulated HGT datasets of the confirmatory test H5 (docs/PREREGISTRATION.md,
# Amendment 4; committed before any of these data were made). For each
# transfer level h = 0, 0.1, 0.2, 0.4 (level index 0 to 3) and replicate
# 1 to 5, with dataset seed D = 7000 + 100 x level index + replicate:
#   1. species tree: AliSim random Yule-Harding tree, 30 taxa,
#      -rlen 0.001 0.01 0.1, seed D;
#   2. gene trees: `qmaws hgt-trees` (1,000 genes of 1,000 sites; exactly
#      round(1000 h) genes get one random SPR of the species tree), seed D;
#   3. genome: AliSim topology-unlinked partitions (-Q), Jukes-Cantor,
#      FASTA, seed D + 50000; trailing spaces in the headers removed.
#
# Usage: sh scripts/simulate_hgt.sh <path to iqtree2> <path to qmaws>
# Writes data/raw/simulated/hgt_<level>_r<rep>/sequences.fasta (not
# committed) and, committed: data/manifests/simulated/hgt.tsv (commands,
# seeds, SHA-256), the species trees in data/manifests/simulated/trees/ and
# the transfer tables in data/manifests/simulated/transfers/.

set -eu

iq=${1:?path to iqtree2}
qmaws=${2:?path to qmaws}
root=$(git rev-parse --show-toplevel)
raw="$root/data/raw/simulated"
man="$root/data/manifests/simulated"
mkdir -p "$raw" "$man/trees" "$man/transfers"
table="$man/hgt.tsv"
printf 'dataset\tfraction\treplicate\tseed\tspecies_command\tgenes_command\tgenome_command\tfasta_sha256\tspecies_tree_sha256\ttransfers_sha256\n' > "$table.tmp"

one() { # level-name fraction level-index replicate
    name=$1; h=$2; li=$3; r=$4
    id="hgt_${name}_r${r}"
    seed=$((7000 + 100 * li + r))
    gseed=$((seed + 50000))
    dir="$raw/$id"
    rm -rf "$dir"
    mkdir -p "$dir"
    c1="iqtree2 --alisim species -t RANDOM{yh/30} -rlen 0.001 0.01 0.1 -m JC --length 10 -seed $seed"
    c2="qmaws hgt-trees --species species.treefile --genes 1000 --gene-length 1000 --fraction $h --seed $seed --output genes"
    c3="iqtree2 --alisim genome -Q genes/partitions.nex -t genes/gene_trees.nwk --out-format fasta -seed $gseed"
    (cd "$dir" && "$iq" --alisim species -t "RANDOM{yh/30}" -rlen 0.001 0.01 0.1 -m JC --length 10 -seed "$seed" > species.log 2>&1)
    (cd "$dir" && "$qmaws" hgt-trees --species species.treefile --genes 1000 --gene-length 1000 --fraction "$h" --seed "$seed" --output genes > genes.log 2>&1)
    (cd "$dir" && "$iq" --alisim genome -Q genes/partitions.nex -t genes/gene_trees.nwk --out-format fasta -seed "$gseed" > genome.log 2>&1)
    sed 's/[[:space:]]*$//' "$dir/genome.fa" > "$dir/sequences.fasta"
    rm "$dir/genome.fa" "$dir/species.phy"
    cp "$dir/species.treefile" "$man/trees/$id.nwk"
    cp "$dir/genes/transfers.tsv" "$man/transfers/$id.tsv"
    fa=$(sha256sum "$dir/sequences.fasta" | cut -d' ' -f1)
    tr=$(sha256sum "$man/trees/$id.nwk" | cut -d' ' -f1)
    tx=$(sha256sum "$man/transfers/$id.tsv" | cut -d' ' -f1)
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$h" "$r" "$seed" "$c1" "$c2" "$c3" "$fa" "$tr" "$tx" >> "$table.tmp"
    echo "$id: seed $seed, $(tail -n +2 "$man/transfers/$id.tsv" | wc -l) transferred genes, sequences SHA-256 $fa"
}

li=0
for level in "h000 0" "h010 0.1" "h020 0.2" "h040 0.4"; do
    set -- $level
    for r in 1 2 3 4 5; do
        one "$1" "$2" "$li" "$r"
    done
    li=$((li + 1))
done
mv "$table.tmp" "$table"
echo "Manifest: $table"
