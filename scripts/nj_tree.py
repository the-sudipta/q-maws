"""Neighbour-joining tree from a distance matrix (Amendment 5; development
only). Used for the tools that give distances, not trees (Mash and CD-MAWS
in Amendment 5; andi, phylonium, FSWM and Skmer in Amendment 8).

Usage:
  python nj_tree.py mash   <mash dist output> <tree out>
  python nj_tree.py csv    <CD-MAWS matrix (header ",a,b,..."; rows "a,d,d,...")> <tree out>
  python nj_tree.py phylip <square PHYLIP matrix (first line: number of taxa)> <tree out>
  python nj_tree.py tsv    <square matrix with a header row (Skmer: "sample\ta\tb...")> <tree out>

Mash names are file paths; the taxon name is the file name without its
extension. The matrix is made symmetric (mean of both entries, which are
equal for both tools) and passed to Biopython's DistanceTreeConstructor("nj").
Branch lengths are dropped from the written Newick (topology only).
"""
import os
import sys

from Bio import Phylo
from Bio.Phylo.TreeConstruction import DistanceMatrix, DistanceTreeConstructor


def taxon(path):
    return os.path.splitext(os.path.basename(path))[0]


def read_mash(path):
    d = {}
    for line in open(path, encoding="utf-8"):
        a, b, dist = line.split("\t")[:3]
        d[(taxon(a), taxon(b))] = float(dist)
    names = sorted({a for a, _ in d})
    return names, d


def read_csv(path):
    lines = [l.rstrip("\n") for l in open(path, encoding="utf-8") if l.strip()]
    names = lines[0].split(",")[1:]
    d = {}
    for line in lines[1:]:
        f = line.split(",")
        for b, v in zip(names, f[1:]):
            d[(f[0], b)] = float(v)
    return names, d


def read_phylip(path):
    rows = [l.split() for l in open(path, encoding="utf-8") if l.strip()]
    n = int(rows[0][0])
    body = rows[1:1 + n]
    names = [taxon(r[0]) for r in body]
    d = {}
    for r, a in zip(body, names):
        for b, v in zip(names, r[1:1 + n]):
            d[(a, b)] = float(v)
    return names, d


def read_tsv(path):
    rows = [l.rstrip("\n").split("\t") for l in open(path, encoding="utf-8") if l.strip()]
    names = [taxon(x) for x in rows[0][1:]]
    d = {}
    for r in rows[1:]:
        for b, v in zip(names, r[1:]):
            d[(taxon(r[0]), b)] = float(v)
    return names, d


READERS = {"mash": read_mash, "csv": read_csv, "phylip": read_phylip, "tsv": read_tsv}


def main():
    kind, src, out = sys.argv[1:4]
    names, d = READERS[kind](src)
    lower = []
    for i, a in enumerate(names):
        row = []
        for b in names[:i]:
            row.append((d[(a, b)] + d[(b, a)]) / 2)
        row.append(0.0)
        lower.append(row)
    tree = DistanceTreeConstructor().nj(DistanceMatrix(names, lower))
    for c in tree.find_clades():
        c.branch_length = None
        if not c.is_terminal():
            c.name = None
    Phylo.write(tree, out, "newick", plain=True)


if __name__ == "__main__":
    main()
