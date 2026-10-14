"""Golden test G9, nRF part (development only; needs Python and DendroPy).

Reads pairs.tsv written by `qmaws metrics-check` and recomputes the
Robinson-Foulds distance of every pair with DendroPy, reading both trees as
unrooted and dividing by 2 (n - 3), as ML-MAWS's run_baselines.sh does.
Exit status 0 when every pair has the same number of differing splits and
the same nRF (within 1e-9) as Q-MAWS.

Usage: python scripts/metrics_check.py <folder with pairs.tsv>
"""

import csv
import os
import sys

import dendropy
from dendropy.calculate import treecompare


def annotate(level, title, message):
    if os.environ.get("GITHUB_ACTIONS"):
        print(f"::{level} title={title}::{message}")
    print(f"{title}: {message}")


def main(folder):
    path = os.path.join(folder, "pairs.tsv")
    with open(path, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    failures = 0
    for row in rows:
        tns = dendropy.TaxonNamespace()
        estimate = dendropy.Tree.get(
            data=row["estimate"], schema="newick", taxon_namespace=tns, rooting="default-unrooted"
        )
        reference = dendropy.Tree.get(
            data=row["reference"], schema="newick", taxon_namespace=tns, rooting="default-unrooted"
        )
        rf = treecompare.symmetric_difference(reference, estimate)
        n = len(tns)
        nrf = rf / (2 * (n - 3)) if n > 3 else 0.0
        if rf != int(row["rf_splits_differ"]) or abs(nrf - float(row["nrf"])) > 1e-9:
            failures += 1
            annotate(
                "error",
                "G9 nRF",
                f"pair {row['pair']} ({n} leaves): DendroPy RF {rf} (nRF {nrf:.9f}), "
                f"Q-MAWS {row['rf_splits_differ']} (nRF {row['nrf']})",
            )
    summary = (
        f"{len(rows) - failures} of {len(rows)} pairs: Q-MAWS nRF equals DendroPy "
        f"{dendropy.__version__} (symmetric difference / 2(n - 3), trees read as unrooted)"
    )
    annotate("notice" if failures == 0 else "error", "G9 nRF", summary)
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    sys.exit(main(sys.argv[1]))
