#!/bin/sh
# Q-MAWS launcher for macOS and Linux.
#
# Looks for the program next to this file (Q-MAWS, or inside Q-MAWS.app on
# macOS: release bundle), then in bin/, then in target/release/ (developer
# build). If neither exists and cargo is available, builds it.
# With no arguments, starts the interactive main menu; otherwise passes all
# arguments through unchanged.

here=$(cd "$(dirname "$0")" && pwd)

find_program() {
    for candidate in "$here/Q-MAWS" "$here/Q-MAWS.app/Contents/MacOS/Q-MAWS"         "$here/bin/qmaws" "$here/target/release/qmaws"; do
        if [ -x "$candidate" ]; then
            program=$candidate
            return 0
        fi
    done
    return 1
}

if ! find_program; then
    if command -v cargo >/dev/null 2>&1; then
        echo "Q-MAWS is not built yet. Building it now with cargo (this can take a few minutes)."
        (cd "$here" && cargo build --release) || {
            echo "Error: the build failed. See the messages above." >&2
            exit 1
        }
        find_program || {
            echo "Error: the build finished but target/release/qmaws was not found." >&2
            exit 1
        }
    else
        echo "Q-MAWS is not installed in this folder, and Rust (cargo) is not available to build it."
        echo ""
        echo "Download the ready-to-run release for your operating system from the"
        echo "Releases page of the Q-MAWS repository:"
        echo "  https://github.com/the-sudipta/q-maws/releases"
        echo "Unpack it, then run ./run.sh again from the unpacked folder."
        exit 1
    fi
fi

if [ $# -eq 0 ]; then
    exec "$program" menu
else
    exec "$program" "$@"
fi
