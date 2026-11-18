#!/bin/sh
# Checks the simulated datasets of the extended benchmark and of M13 against the committed
# manifests (data/manifests/simulated/benchmark.tsv and scaling.tsv), by SHA-256 of the
# sequences (for fragmented assemblies: of all their files joined in name order, as in
# scripts/simulate_benchmark.sh).
#
# Usage (from the repository folder): sh scripts/cloud/check_simulated.sh <output tsv>
# Writes dataset, expected SHA-256, SHA-256 found, equal (yes/no/missing); exit 1 if any
# dataset is not equal.

set -u
out=${1:?output tsv}
man=data/manifests/simulated
raw=data/raw/simulated
LC_ALL=C; export LC_ALL       # the name order of the fragment files

found() { # dataset -> SHA-256 of its sequences, or "missing"
    case $1 in
        bm_frag_*) ls "$raw/$1"/*.fasta > /dev/null 2>&1 && cat "$raw/$1"/*.fasta | sha256sum | cut -d' ' -f1 || echo missing ;;
        *) [ -f "$raw/$1/sequences.fasta" ] && sha256sum "$raw/$1/sequences.fasta" | cut -d' ' -f1 || echo missing ;;
    esac
}

{
    printf 'dataset\texpected_sha256\tfound_sha256\tequal\n'
    for spec in "benchmark.tsv 11" "scaling.tsv 7"; do
        set -- $spec
        awk -F'\t' -v c="$2" 'NR > 1 { print $1, $c }' "$man/$1" | while read -r id want; do
            case $id in scaling_m200_*) continue ;; esac     # left out (Amendment 7)
            got=$(found "$id")
            ok=no; [ "$got" = "$want" ] && ok=yes; [ "$got" = missing ] && ok=missing
            printf '%s\t%s\t%s\t%s\n' "$id" "$want" "$got" "$ok"
        done
    done
} > "$out"
total=$(($(wc -l < "$out") - 1)); equal=$(grep -c 'yes$' "$out")
echo "simulated datasets equal to the manifests: $equal of $total"
[ "$equal" -eq "$total" ]
