#!/bin/sh
# README coverage check.
#
# Every folder that contains tracked or untracked (not ignored) files must have
# a README.md, and every item directly inside a folder must be listed in that
# README as `name` (file) or `name/` (folder). The repository root README and
# each README itself are not required to list themselves.
#
# Exception: folders under a crate's src/ that contain a single file need no
# README.
#
# Exit status 0 means every folder is covered.

root=$(git rev-parse --show-toplevel) || exit 1
cd "$root" || exit 1

files=$(git -c core.quotePath=false ls-files --cached --others --exclude-standard)
dirs=$(printf '%s\n' "$files" | sed -n 's|/[^/]*$||p' | awk -F/ '{
    path = $1; print path
    for (i = 2; i <= NF; i++) { path = path "/" $i; print path }
}' | sort -u)

failed=0

check_dir() {
    dir=$1 # "." for the root
    if [ "$dir" = "." ]; then
        prefix=""
    else
        prefix="$dir/"
    fi
    children=$(printf '%s\n' "$files" | awk -v p="$prefix" '
        index($0, p) == 1 {
            rest = substr($0, length(p) + 1)
            n = index(rest, "/")
            if (n > 0) print substr(rest, 1, n) ; else print rest
        }' | sort -u)
    count=$(printf '%s\n' "$children" | grep -c .)

    case "$dir" in
        crates/*/src | crates/*/src/*)
            [ "$count" -le 1 ] && return 0
            ;;
    esac

    readme="${prefix}README.md"
    if [ ! -f "$readme" ]; then
        echo "Missing README: ${prefix:-./}"
        failed=1
        return 0
    fi

    old_ifs=$IFS
    IFS='
'
    for child in $children; do
        [ "$child" = "README.md" ] && continue
        if ! grep -Fq -- "\`$child\`" "$readme"; then
            echo "Not listed in $readme: $child"
            failed=1
        fi
    done
    IFS=$old_ifs
}

check_dir "."
old_ifs=$IFS
IFS='
'
for d in $dirs; do
    IFS=$old_ifs
    check_dir "$d"
done
IFS=$old_ifs

if [ "$failed" -eq 0 ]; then
    echo "README coverage: every folder is covered."
fi
exit $failed
