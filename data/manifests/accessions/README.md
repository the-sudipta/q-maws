# accessions

## Purpose
Accession lists of the five NCBI benchmark datasets. `qmaws download` fetches exactly these record versions from NCBI Nucleotide and writes them, under the names given here, into one multi-FASTA file per dataset.

## Contents
| Item | Description |
|---|---|
| `coronavirus.tsv` | 34 accessions: 30 coronaviruses and 4 outgroup viruses (Table S5) |
| `ebolavirus.tsv` | 59 Ebolavirus genomes (Table S4) |
| `influenza_a.tsv` | 38 influenza A viruses, segment 6 (neuraminidase) (Table S2) |
| `mammal_mtdna.tsv` | 41 mammalian mitochondrial genomes (Table S1) |
| `rhinovirus.tsv` | 113 human rhinovirus genomes and 3 outgroup genomes (Table S3) |

## Relationships
Compiled into `crates/qmaws-data` (`registry.rs`) and named by the `accessions` field of the NCBI downloads in `../benchmarks.toml`. The fetched files go to `data/raw/<id>/<id>.fasta`, with a provenance table `<id>.accessions.tsv` (name, accession, version, NCBI title) next to them.

## Notes

**Columns:** `accession` (as printed in the source table), `name` (the taxon name, written as the FASTA header; unsafe Newick characters become `_` when read), `table_text` (the table row as printed, without the length column), `version` (the exact record version fetched and verified on 2026-10-02).

**Source:** Supplementary Information of Y. Li, L. He, R. L. He and S. S.-T. Yau, "A novel fast vector method for genetic sequence comparison", Scientific Reports 7:12226 (2017), https://doi.org/10.1038/s41598-017-12493-2 (open access, CC BY 4.0). File `41598_2017_12493_MOESM1_ESM.pdf` (1,808,034 bytes, SHA-256 `d4c5b48e10529cb65a0f3af8b69c17d583a71361f6f8c65aa98e1115dd58945b`), downloaded on 2026-10-02 from the article page. Tables S1 to S5 were converted to text with `pdftotext -raw` (Xpdf 4.06); rows were split at each accession number. Two corrections of the conversion: page-break characters before some accessions were removed, and one strain name broken across two lines was rejoined (`A/American green-winged teal/California/44242-906/2007(H7N3)`).

**Names:** the abbreviation column for Tables S3, S4 and S5 (for example `1_HCoV-229E`, `EBOV_1995_AY354458`, `HEV_cva-13*`); the description for Table S2 (strain name with subtype); the description for Table S1 (common name).

**Verification (2026-10-02):**
- NCBI returned a record for every accession (288 in total): version 1 for 268 records, version 2 for 19, version 3 for one.
- Every fetched sequence is identical, after cleaning, to the sequence ML-MAWS used (data files in the ML-MAWS repository, commit `0c38db1`): 34 of 34, 59 of 59, 38 of 38, 41 of 41, 116 of 116 (`qmaws inspect --dataset <id> --compare-with <id>_mlmaws`). Each identical pair also has the same name, except five mammals written differently in the two sources (for example `Indian_Rhinoceros` and `Indian_Rhino`), which confirms that each accession was paired with the right name.
- All 38 influenza records are segment 6 (neuraminidase) according to their NCBI titles; 35 titles contain the table name exactly, and the other 3 differ only by abbreviation (`MD` for `Maryland`, `07` for `2007`, `83` for `1983`).
- The file built with pinned versions was rebuilt once more from NCBI and was byte-identical (same SHA-256).

**Not used from the supplement:** the genome lengths of Table S4 decrease by exactly 1 from row to row (18,941 to 18,883), which looks like an artifact of the table, not measured lengths. They are not used; lengths are measured from the sequences.
