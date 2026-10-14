#!/bin/sh
# Peer-reviewed alignment-free tools on one dataset (docs/PREREGISTRATION.md,
# Amendment 5; development only; Linux x86-64 with g++, git, curl, python3,
# apt-get and network access). Accuracy only: run times measured here are
# recorded but not compared with anything.
#
# Usage: sh scripts/baselines.sh <tool> <dataset> <output folder> [work folder]
#   tool:    mash | cdmaws | peafowl
#   dataset: fish_mito | ecoli | ecoli_shigella_hgt | yersinia_hgt |
#            sim_hgt_0 ... sim_hgt_1000 | hgt_h000_r1 ... hgt_h040_r5 (H5)
# Writes to <output folder>: tree.nwk, comparison.tsv (qmaws tree-compare
# against the reference or species tree), the tool's messages and
# provenance.txt. Exit 0 also when the tool fails: the failure is recorded in
# provenance.txt (status) so that it is reported, not left out.

set -u

MASH_VERSION=2.3
MASH_URL="https://github.com/marbl/Mash/releases/download/v$MASH_VERSION/mash-Linux64-v$MASH_VERSION.tar"
CDMAWS_REPO=https://github.com/TamimEhsan/cd-maws-sa
CDMAWS_COMMIT=8bfe7d55412af0f211e969f85adf9d20f0d3af53
PEAFOWL_REPO=https://github.com/hasin-abrar/Peafowl-repo
PEAFOWL_COMMIT=89ed477bba1072cef891b7304896891d233edd91
IQ_VERSION=2.4.0
IQ_URL="https://github.com/iqtree/iqtree2/releases/download/v$IQ_VERSION/iqtree-$IQ_VERSION-Linux-intel.tar.gz"
BIOPYTHON=1.84
LIMIT_MINUTES=330

tool=${1:?tool}
dataset=${2:?dataset}
out=${3:?output folder}
root=$(git rev-parse --show-toplevel) || exit 1
work=${4:-"$root/../_external"}
mkdir -p "$work" "$out"
work=$(cd "$work" && pwd)
out=$(cd "$out" && pwd)
qmaws="$root/target/release/qmaws"
threads=$(nproc)
status=ok
note=""

annotate() { # level title message
    [ -n "${GITHUB_ACTIONS:-}" ] && echo "::$1 title=$2::$3"
    echo "$2: $3"
}
fail() { status="$1"; note="$2"; annotate warning "$tool $dataset" "$2"; }

# 1. The input: a folder with one FASTA file per taxon (file name = taxon
#    name, as in the reference trees) and the reference tree.
inp="$work/input_$dataset"
rm -rf "$inp"
mkdir -p "$inp"
case "$dataset" in
    hgt_h*)
        # H5 data, made again with the committed script and checked by SHA-256.
        archive="$work/iqtree-$IQ_VERSION.tar.gz"
        [ -f "$archive" ] || curl -sSfL -o "$archive" "$IQ_URL" || exit 1
        tar -xzf "$archive" -C "$work" || exit 1
        iq=$(find "$work" -type f -name iqtree2 | head -n 1)
        chmod +x "$iq"
        level=${dataset#hgt_}; level=${level%_r*}; rep=${dataset##*_r}
        case "$level" in h000) h=0; li=0 ;; h010) h=0.1; li=1 ;; h020) h=0.2; li=2 ;; h040) h=0.4; li=3 ;; esac
        seed=$((7000 + 100 * li + rep))
        sim="$work/sim_$dataset"
        rm -rf "$sim"; mkdir -p "$sim"
        (cd "$sim" && "$iq" --alisim species -t "RANDOM{yh/30}" -rlen 0.001 0.01 0.1 -m JC --length 10 -seed "$seed" > species.log 2>&1 \
            && "$qmaws" hgt-trees --species species.treefile --genes 1000 --gene-length 1000 --fraction "$h" --seed "$seed" --output genes > genes.log 2>&1 \
            && "$iq" --alisim genome -Q genes/partitions.nex -t genes/gene_trees.nwk --out-format fasta -seed $((seed + 50000)) > genome.log 2>&1) || exit 1
        sed 's/[[:space:]]*$//' "$sim/genome.fa" > "$sim/sequences.fasta"
        want=$(awk -F'\t' -v d="$dataset" '$1 == d { print $8 }' "$root/data/manifests/simulated/hgt.tsv")
        got=$(sha256sum "$sim/sequences.fasta" | cut -d' ' -f1)
        if [ "$got" != "$want" ]; then
            fail "not run (data differ)" "sequences.fasta SHA-256 $got differs from the manifest value $want"
        fi
        awk -v d="$inp" '/^>/ { f = d "/" substr($1, 2) ".fasta" } { print > f }' "$sim/sequences.fasta"
        reference="$root/data/manifests/simulated/trees/$dataset.nwk"
        ;;
    *)
        "$qmaws" --quiet download --dataset "${dataset%_[0-9]*}" --data-dir "$root/data" > /dev/null 2>&1 \
            || "$qmaws" --quiet download --dataset "$dataset" --data-dir "$root/data" || exit 1
        case "$dataset" in
            fish_mito) src="$root/data/raw/fish_mito/assembled-fish_mito"; reference="$root/data/references/fish_mito.nwk" ;;
            ecoli) src="$root/data/raw/ecoli/assembled-ecoli"; reference="$root/data/references/ecoli.nwk" ;;
            ecoli_shigella_hgt) src="$root/data/raw/ecoli_shigella_hgt/unsimulated-ecoli_shigella"; reference="$root/data/references/ecoli_shigella_hgt.nwk" ;;
            yersinia_hgt) src="$root/data/raw/yersinia_hgt/unsimulated-yersinia"; reference="$root/data/references/yersinia_hgt.nwk" ;;
            sim_hgt_*) src="$root/data/raw/sim_hgt/simulated-sim_hgt/hgt_${dataset#sim_hgt_}"; reference="$root/data/references/sim_hgt.nwk" ;;
            *) annotate error "$dataset" "unknown dataset"; exit 1 ;;
        esac
        for f in "$src"/*.fasta; do cp "$f" "$inp/"; done
        ;;
esac
taxa=$(ls "$inp" | wc -l)

# 2. The tool.
run="$work/run_${tool}_$dataset"
rm -rf "$run"; mkdir -p "$run"
version=""
start=$(date +%s)
if [ "$status" = ok ]; then
case "$tool" in
    mash)
        bin="$work/mash-Linux64-v$MASH_VERSION/mash"
        if [ ! -x "$bin" ]; then
            curl -sSfL -o "$work/mash.tar" "$MASH_URL" && tar -xf "$work/mash.tar" -C "$work" || exit 1
        fi
        version="Mash $("$bin" --version) ($MASH_URL, SHA-256 $(sha256sum "$work/mash.tar" | cut -d' ' -f1)); defaults k = 21, sketch size 1000"
        if timeout "${LIMIT_MINUTES}m" "$bin" sketch -p "$threads" -o "$run/sketch" "$inp"/*.fasta > "$run/stdout.txt" 2> "$run/stderr.txt" \
            && timeout "${LIMIT_MINUTES}m" "$bin" dist -p "$threads" "$run/sketch.msh" "$run/sketch.msh" > "$run/distances.tsv" 2>> "$run/stderr.txt"; then
            python3 "$root/scripts/nj_tree.py" mash "$run/distances.tsv" "$run/tree.nwk" || fail failed "neighbour joining failed"
        else
            fail failed "mash exit $? (124 = time limit)"
        fi
        ;;
    cdmaws)
        src="$work/cd-maws-sa"
        [ -d "$src/.git" ] || git clone -q "$CDMAWS_REPO" "$src" || exit 1
        (cd "$src" && git checkout -q "$CDMAWS_COMMIT" && g++ -O2 -o cd-maws-sa/maw-gen cd-maws-sa/maw-gen.cpp && g++ -O2 -o cd-maws-sa/dist-calc cd-maws-sa/dist-calc.cpp) \
            > "$run/build.txt" 2>&1 || exit 1
        version="cd-maws-sa commit $CDMAWS_COMMIT (maw-gen <file> <out> 4 14; dist-calc); $(g++ --version | head -n 1)"
        : > "$run/maws.txt"
        ok=1
        for f in "$inp"/*.fasta; do
            name=$(basename "$f" .fasta)
            # one sequence per taxon: contigs joined, letters other than A C G T removed, header = taxon name
            { echo ">$name"; grep -v '^>' "$f" | tr 'acgt' 'ACGT' | tr -cd 'ACGT'; echo; } > "$run/one.fasta"
            timeout "${LIMIT_MINUTES}m" "$src/cd-maws-sa/maw-gen" "$run/one.fasta" "$run/one.maw" 4 14 2>> "$run/stderr.txt" || { ok=0; break; }
            cat "$run/one.maw" >> "$run/maws.txt"
        done
        rm -f "$run/one.fasta" "$run/one.maw"
        if [ $ok = 1 ] && timeout "${LIMIT_MINUTES}m" "$src/cd-maws-sa/dist-calc" "$run/maws.txt" "$run/distances.csv" 2>> "$run/stderr.txt"; then
            python3 "$root/scripts/nj_tree.py" csv "$run/distances.csv" "$run/tree.nwk" || fail failed "neighbour joining failed"
        else
            fail failed "cd-maws-sa failed or reached the time limit; see stderr.txt"
        fi
        rm -f "$run/maws.txt"
        ;;
    peafowl)
        src="$work/Peafowl-repo"
        [ -d "$src/.git" ] || git clone -q "$PEAFOWL_REPO" "$src" || exit 1
        (cd "$src" && git checkout -q "$PEAFOWL_COMMIT") || exit 1
        # Peafowl calls raxmlHPC-PTHREADS; the Ubuntu package may name it differently.
        mkdir -p "$work/bin"
        rx=$(command -v raxmlHPC-PTHREADS || command -v raxmlHPC-PTHREADS-SSE3 || command -v raxmlHPC-PTHREADS-AVX2 || command -v raxmlHPC-PTHREADS-AVX)
        [ -n "$rx" ] || { annotate error peafowl "no RAxML binary"; exit 1; }
        ln -sf "$rx" "$work/bin/raxmlHPC-PTHREADS"
        PATH="$work/bin:$PATH"; export PATH
        version="Peafowl commit $PEAFOWL_COMMIT (peafowl.sh -i <folder> -n $threads -r); jellyfish $(jellyfish --version 2>&1 | head -n 1); RAxML $("$rx" -v 2>/dev/null | grep -m 1 -o 'version [0-9.]*') ($rx)"
        # Peafowl works in its own folder, on a sibling input folder without dots in names.
        pw="$run/peafowl"
        cp -r "$src" "$pw"
        cp -r "$inp" "$pw/input"
        if (cd "$pw" && timeout "${LIMIT_MINUTES}m" bash ./peafowl.sh -i input -n "$threads" -r) > "$run/stdout.txt" 2> "$run/stderr.txt"; then
            t=$(ls "$pw"/input_Result/Result_unrooted_tree_Kmer_*.newick 2>/dev/null | head -n 1)
            if [ -n "$t" ]; then
                cp "$t" "$run/tree.nwk"
                version="$version; k = $(basename "$t" .newick | sed 's/.*_Kmer_//')"
            else
                fail failed "no Peafowl tree written; see stdout.txt and stderr.txt"
            fi
        else
            fail failed "peafowl.sh exit $? (124 = time limit)"
        fi
        rm -rf "$pw"
        ;;
    *) annotate error "$tool" "unknown tool"; exit 1 ;;
esac
fi
seconds=$(( $(date +%s) - start ))

# 3. Outputs and the comparison with the reference.
for f in tree.nwk distances.tsv distances.csv stdout.txt stderr.txt build.txt; do
    [ -f "$run/$f" ] && cp "$run/$f" "$out/"
done
if [ -f "$out/tree.nwk" ]; then
    "$qmaws" --quiet tree-compare --tree "$out/tree.nwk" --reference "$reference" > "$out/comparison.tsv" 2> "$out/comparison_error.txt" \
        || fail failed "tree-compare failed; see comparison_error.txt"
elif [ "$status" = ok ]; then
    fail failed "no tree"
fi
{
    echo "tool: $tool"
    echo "dataset: $dataset ($taxa taxa)"
    echo "reference: ${reference#$root/}"
    echo "status: $status"
    echo "note: $note"
    echo "version: $version"
    echo "biopython: $(python3 -c 'import Bio; print(Bio.__version__)' 2>/dev/null)"
    echo "wall_seconds: $seconds (GitHub runner; not compared)"
    echo "threads: $threads"
    echo "cpu: $(grep -m 1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ *//')"
    echo "date_utc: $(date -u +%FT%TZ)"
} > "$out/provenance.txt"
annotate notice "$tool $dataset" "status $status; $(grep -m 1 '^nRF' "$out/comparison.tsv" 2>/dev/null | cut -f2 | sed 's/^/nRF /')"
exit 0
