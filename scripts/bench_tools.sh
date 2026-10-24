#!/bin/bash
# Runs one alignment-free method of the extended benchmark (docs/PREREGISTRATION.md,
# Amendment 8) inside Ubuntu (WSL) on the development laptop, and writes its tree.
#
# Usage (inside WSL, as root):
#   bash bench_tools.sh <method> <input> <output folder> [setting]
# <input> is a folder with one FASTA file per taxon (several records = contigs
# of one genome) or one multi-FASTA file with one record per taxon.
# [setting] is the tuned value (word length k or pattern weight); without it
# the method's published default is used.
# Writes <output>/tree.nwk (leaf names = taxon names, topology only),
# <output>/version.txt and <output>/command.txt; returns 0 on success.
#
# Tools live in /opt/bench (see Amendment 8b for versions and commits).

set -u
method=${1:?method}; input=${2:?input}; out=${3:?output folder}; setting=${4:-}
B=/opt/bench
T=12
LIMIT=360m
here=$(cd "$(dirname "$0")" && pwd)
export PATH="$B/bin:$PATH"
PY=$B/venv/bin/python
mkdir -p "$out"
w=$(mktemp -d /tmp/bench_XXXXXX)
trap 'rm -rf "$w"' EXIT

# --- input: one file per taxon in $w/genomes, and one joined multi-FASTA ---
mkdir -p "$w/genomes"
if [ -f "$input" ]; then
    awk -v d="$w/genomes" '/^>/{name=substr($1,2); f=d"/"name".fasta"; print ">"name > f; next} {print >> f}' "$input"
else
    for f in "$input"/*.fasta "$input"/*.fa "$input"/*.fna "$input"/*.fas; do
        [ -f "$f" ] || continue
        b=$(basename "$f"); b=${b%.*}
        cp "$f" "$w/genomes/$b.fasta"
    done
fi
n=$(ls "$w/genomes" | wc -l)
[ "$n" -ge 4 ] || { echo "fewer than 4 taxa in $input" >&2; exit 2; }
: > "$w/joined.fasta"
for f in "$w"/genomes/*.fasta; do
    b=$(basename "$f" .fasta)
    { echo ">$b"; grep -v '^>' "$f" | tr 'acgt' 'ACGT' | tr -cd 'ACGT'; echo; } >> "$w/joined.fasta"
done

record() { echo "$1" > "$out/version.txt"; echo "$2" > "$out/command.txt"; }
nj() { "$PY" "$here/nj_tree.py" "$1" "$2" "$w/nj.nwk" && cp "$w/nj.nwk" "$out/tree.nwk"; }
run() { timeout "$LIMIT" "$@"; }

case "$method" in
mash) # Mash 2.3 (official binary), defaults k = 21 and sketch size 1,000; NJ
    k=${setting:-21}
    record "Mash $("$B/mash-Linux64-v2.3/mash" --version)" "mash sketch -k $k -s 1000; mash dist; NJ (Biopython 1.84)"
    run "$B/mash-Linux64-v2.3/mash" sketch -p $T -k "$k" -s 1000 -o "$w/sk" "$w"/genomes/*.fasta > "$out/stdout.txt" 2> "$out/stderr.txt" &&
    run "$B/mash-Linux64-v2.3/mash" dist -p $T "$w/sk.msh" "$w/sk.msh" > "$w/d.tsv" 2>> "$out/stderr.txt" && nj mash "$w/d.tsv" ;;
cdmaws) # CD-MAWS (suffix automaton), MAW lengths 4 to 14 as in the authors' script; NJ
    record "cd-maws-sa commit $(cd $B/cd-maws-sa && git rev-parse --short HEAD)" "maw-gen <taxon> 4 14; dist-calc; NJ (Biopython 1.84)"
    : > "$w/maws.txt"
    awk -v d="$w" '/^>/{f=d"/one_"substr($1,2)".fasta"} {print > f}' "$w/joined.fasta"
    ok=1
    for f in "$w"/one_*.fasta; do
        run "$B/cd-maws-sa/cd-maws-sa/maw-gen" "$f" "$w/one.maw" 4 14 2>> "$out/stderr.txt" || { ok=0; break; }
        cat "$w/one.maw" >> "$w/maws.txt"
    done
    [ $ok = 1 ] && run "$B/cd-maws-sa/cd-maws-sa/dist-calc" "$w/maws.txt" "$w/d.csv" 2>> "$out/stderr.txt" && nj csv "$w/d.csv" ;;
peafowl) # Peafowl, default script (k chosen by entropy); runs in its own folder
    record "Peafowl commit $(cd $B/Peafowl-repo && git rev-parse --short HEAD); jellyfish $(jellyfish --version | cut -d' ' -f2); $(raxmlHPC -v | grep -m1 -o 'version [0-9.]*')" "peafowl.sh -i input -n $T -r"
    cp -r "$B/Peafowl-repo" "$w/pw"; cp -r "$w/genomes" "$w/pw/input"
    (cd "$w/pw" && run bash ./peafowl.sh -i input -n $T -r) > "$out/stdout.txt" 2> "$out/stderr.txt"
    t=$(ls "$w"/pw/input_Result/Result_unrooted_tree_Kmer_*.newick 2>/dev/null | head -n 1)
    [ -n "$t" ] && cp "$t" "$out/tree.nwk" ;;
andi) # andi 0.14, defaults (JC model); contigs of one file are one genome (-j); NJ
    record "$(andi --version 2>&1 | head -1)" "andi -j -t $T <files>; NJ (Biopython 1.84)"
    run andi -j -t $T "$w"/genomes/*.fasta > "$w/d.phy" 2> "$out/stderr.txt" && nj phylip "$w/d.phy" ;;
phylonium) # phylonium 1.7, defaults; NJ
    record "$(phylonium --version 2>&1 | head -1)" "phylonium -t $T <files>; NJ (Biopython 1.84)"
    run phylonium -t $T "$w"/genomes/*.fasta > "$w/d.phy" 2> "$out/stderr.txt" && nj phylip "$w/d.phy" ;;
fswm) # FSWM, default pattern weight 12; one record per genome; NJ
    k=${setting:-12}
    record "FSWM commit $(cd $B/FSWM && git rev-parse --short HEAD)" "fswm -k $k -t $T joined.fasta; NJ (Biopython 1.84)"
    (cd "$w" && run "$B/FSWM/fswm" -k "$k" -t $T joined.fasta) > "$out/stdout.txt" 2> "$out/stderr.txt" && nj phylip "$w/DMat" ;;
multispam) # Multi-SpaM, defaults (weight 10, 100 don't-care positions, 1,000,000 quartet blocks)
    k=${setting:-10}
    record "Multi-SpaM commit $(cd $B/multi-SpaM && git rev-parse --short HEAD)" "multispam.py -t $T -k $k -i joined.fasta -o tree"
    cp -r "$B/multi-SpaM" "$w/ms"
    (cd "$w/ms" && run python3 multispam.py -t $T -k "$k" -i "$w/joined.fasta" -o "$w/ms_tree.nwk") > "$out/stdout.txt" 2> "$out/stderr.txt" &&
    cp "$w/ms_tree.nwk" "$out/tree.nwk" ;;
sans) # SANS (ambages, successor of SANS serif), default k = 31; tree-compatible splits (-f strict)
    k=${setting:-31}
    record "SANS commit $(cd $B/sans && git rev-parse --short HEAD)" "SANS -i list -k $k -f strict -N tree -T $T"
    ls "$w"/genomes/*.fasta > "$w/list.txt"
    run "$B/sans/SANS" -i "$w/list.txt" -k "$k" -f strict -o "$w/splits.tsv" -N "$w/sans.nwk" -T $T > "$out/stdout.txt" 2> "$out/stderr.txt" &&
    cp "$w/sans.nwk" "$out/tree.nwk" ;;
cvtree) # CVTree 3 (Hao method), nucleotide genomes; default K list 5 6 7 -> the tree of K = 7, or the tuned K
    k=${setting:-7}
    record "CVTree commit $(cd $B/cvtree && git rev-parse --short HEAD)" "cvtree -G genomes -g fna -i list -k $k -m Hao"
    mkdir -p "$w/cv"; for f in "$w"/genomes/*.fasta; do b=$(basename "$f" .fasta); cp "$f" "$w/cv/$b.fna"; echo "$b" >> "$w/cv/list"; done
    (cd "$w/cv" && run "$B/cvtree/build/bin/cvtree" -G . -g fna -i list -k "$k" -m Hao -t "$w/cv.nwk" -q) > "$out/stdout.txt" 2> "$out/stderr.txt" &&
    cp "$w/cv.nwk" "$out/tree.nwk" ;;
jolytree) # JolyTree 2.1, defaults (k and sketch size estimated; BME tree; no branch support needed)
    record "JolyTree commit $(cd $B/JolyTree && git rev-parse --short HEAD); FastME commit $(cd $B/FastME && git rev-parse --short HEAD); REQ commit $(cd $B/REQ && git rev-parse --short HEAD)" "JolyTree.sh -i genomes -b jt -t $T -x"
    (cd "$w" && run bash "$B/JolyTree/JolyTree.sh" -i genomes -b jt -t $T -x) > "$out/stdout.txt" 2> "$out/stderr.txt" &&
    cp "$w/jt.nwk" "$out/tree.nwk" ;;
skmer) # Skmer, defaults k = 31, sketch size 100,000; distances; NJ
    k=${setting:-31}
    record "Skmer commit $(cd $B/Skmer && git rev-parse --short HEAD)" "skmer reference genomes -k $k -p $T; NJ (Biopython 1.84)"
    (cd "$w" && run "$B/skmer-venv/bin/skmer" reference genomes -k "$k" -p $T -l lib -o dist) > "$out/stdout.txt" 2> "$out/stderr.txt" &&
    nj tsv "$w/dist.txt" ;;
mashtree) # mashtree 1.4.6, defaults (k = 21, sketch size 10,000); NJ inside mashtree
    k=${setting:-21}
    record "$(mashtree --version 2>&1 | head -1)" "mashtree --numcpus $T --kmerlength $k <files>"
    run mashtree --numcpus $T --kmerlength "$k" "$w"/genomes/*.fasta > "$w/mt.nwk" 2> "$out/stderr.txt" && cp "$w/mt.nwk" "$out/tree.nwk" ;;
aaf) # AAF, default k = 25; one data folder per species; Fitch tree (AAF's own)
    k=${setting:-25}
    record "AAF commit $(cd $B/AAF && git rev-parse --short HEAD) (BetaVersion)" "aaf_phylokmer.py -k $k -t $T; aaf_distance.py -t $T"
    cp -r "$B/AAF/BetaVersion" "$w/aaf"; rm -rf "$w/aaf/data"; mkdir -p "$w/aaf/data"
    for f in "$w"/genomes/*.fasta; do b=$(basename "$f" .fasta); mkdir -p "$w/aaf/data/$b"; cp "$f" "$w/aaf/data/$b/$b.fa"; done
    (cd "$w/aaf" && export PATH="$w/aaf/phylokmer:$w/aaf/phylip_src:$PATH" &&
     run python3 aaf_phylokmer.py -k "$k" -t $T -d data -o pk && run python3 aaf_distance.py -i pk.dat.gz -t $T -o aafd) > "$out/stdout.txt" 2> "$out/stderr.txt"
    t=$(ls "$w"/aaf/aafd*.tre "$w"/aaf/*.tre 2>/dev/null | head -n 1)
    [ -n "$t" ] && cp "$t" "$out/tree.nwk" ;;
*) echo "unknown method: $method" >&2; exit 2 ;;
esac

[ -s "$out/tree.nwk" ] || { echo "no tree written" >&2; exit 1; }
# leaf names = taxon names (strip folders and extensions), topology only
"$PY" - "$out/tree.nwk" <<'PYEOF'
import os, sys
from Bio import Phylo
p = sys.argv[1]
t = Phylo.read(p, "newick")
for c in t.find_clades():
    c.branch_length = None
    if c.is_terminal():
        n = os.path.basename(str(c.name).strip("'\""))
        for ext in (".fasta", ".fa", ".fna", ".fas"):
            if n.endswith(ext):
                n = n[: -len(ext)]
        c.name = n
    else:
        c.name = None
        c.confidence = None
Phylo.write(t, p, "newick", plain=True)
PYEOF
