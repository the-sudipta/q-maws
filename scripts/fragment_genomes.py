"""Cut each genome of a multi-FASTA file into contigs (extended benchmark, Amendment 8).

Usage: python scripts/fragment_genomes.py <sequences.fasta> <output folder> <seed>

Each record of the input is one taxon. Its sequence is cut into consecutive
contigs whose lengths are drawn uniformly from 2,000 to 18,000 letters (mean
10,000); the last contig keeps the remainder. Each taxon is written as one
multi-record file <taxon>.fasta in the output folder, contigs named
<taxon>_contig<k>. The random generator is seeded with <seed> plus the index
of the taxon, so the result does not depend on the order of the files read
later.
"""
import os
import random
import sys


def read_fasta(path):
    name, parts = None, []
    with open(path, encoding='ascii') as f:
        for line in f:
            line = line.strip()
            if line.startswith('>'):
                if name is not None:
                    yield name, ''.join(parts)
                name, parts = line[1:].split()[0], []
            elif line:
                parts.append(line)
    if name is not None:
        yield name, ''.join(parts)


def main():
    src, out, seed = sys.argv[1], sys.argv[2], int(sys.argv[3])
    os.makedirs(out, exist_ok=True)
    for i, (name, seq) in enumerate(read_fasta(src)):
        rng = random.Random(seed * 1000 + i)
        pos, k = 0, 0
        with open(os.path.join(out, name + '.fasta'), 'w', encoding='ascii', newline='\n') as f:
            while pos < len(seq):
                k += 1
                n = rng.randint(2000, 18000)
                f.write(f'>{name}_contig{k}\n')
                chunk = seq[pos:pos + n]
                for j in range(0, len(chunk), 80):
                    f.write(chunk[j:j + 80] + '\n')
                pos += n
        print(f'{name}: {k} contigs')


if __name__ == '__main__':
    main()
