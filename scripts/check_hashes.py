"""Checks every `<file>.sha256` under the given folders: recomputes the
SHA-256 of `<file>` and compares it with the hash stored in `<file>.sha256`
(64 hexadecimal characters). Works on Windows, macOS and Linux with the
Python 3 standard library only.

Usage:
  python scripts/check_hashes.py                 # every .sha256 under results/
  python scripts/check_hashes.py results/runs/fish_mito_seed2 results/h5
  python scripts/check_hashes.py --quiet results # only the summary and failures

Exit status 0 if every file matches, 1 if any file differs or is missing.
"""
import hashlib
import os
import sys


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def main(argv):
    quiet = '--quiet' in argv
    folders = [a for a in argv if a != '--quiet'] or ['results']
    checked = failed = 0
    for folder in folders:
        for root, dirs, files in os.walk(folder):
            dirs.sort()
            for name in sorted(files):
                if not name.endswith('.sha256'):
                    continue
                target = os.path.join(root, name[:-len('.sha256')])
                with open(os.path.join(root, name), encoding='ascii') as f:
                    stored = f.read().split()[0].lower()
                checked += 1
                if not os.path.exists(target):
                    failed += 1
                    print(f'MISSING  {target}')
                    continue
                actual = sha256_of(target)
                if actual != stored:
                    failed += 1
                    print(f'DIFFERS  {target}\n         stored   {stored}\n         computed {actual}')
                elif not quiet:
                    print(f'OK       {target}')
    print(f'{checked} files checked, {checked - failed} match, {failed} differ or are missing')
    return 1 if failed or checked == 0 else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
