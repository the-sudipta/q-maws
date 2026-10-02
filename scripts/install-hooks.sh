#!/bin/sh
# Install the Q-MAWS pre-commit guard as the pre-commit and commit-msg hooks
# of the current repository. Run from anywhere inside the repository.
set -e

root=$(git rev-parse --show-toplevel)
hooks=$(git rev-parse --git-path hooks)
mkdir -p "$hooks"

for hook in pre-commit commit-msg; do
    cp "$root/scripts/pre-commit" "$hooks/$hook"
    chmod +x "$hooks/$hook"
    echo "Installed $hooks/$hook"
done
