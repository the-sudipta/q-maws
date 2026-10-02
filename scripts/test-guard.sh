#!/bin/sh
# Self-test for the pre-commit guard (scripts/pre-commit).
#
# Creates a throw-away git repository in a temporary folder, installs the
# guard as its pre-commit and commit-msg hooks, and checks that each blocked
# case is blocked and that a clean commit passes. The real repository is not
# touched. Exit status 0 means every case behaved as expected.
#
# Forbidden strings are assembled from fragments at run time so that this
# file does not itself contain them.

set -u

root=$(git rev-parse --show-toplevel)
guard="$root/scripts/pre-commit"
tmp=$(mktemp -d 2>/dev/null || mktemp -d -t qmaws-guard)
trap 'rm -rf "$tmp"' EXIT INT TERM

passed=0
failed=0

git -C "$tmp" init -q
git -C "$tmp" config user.name "Guard Test"
git -C "$tmp" config user.email "guard-test@example.invalid"
git -C "$tmp" config core.autocrlf false
hooks="$tmp/.git/hooks"
cp "$guard" "$hooks/pre-commit"
cp "$guard" "$hooks/commit-msg"
chmod +x "$hooks/pre-commit" "$hooks/commit-msg"

# expect <allowed|blocked> <description> [git add options] <path>
# Stages <path> (already created in $tmp), tries to commit, checks the result,
# then resets the index and removes the file.
expect() {
    want=$1
    what=$2
    shift 2
    git -C "$tmp" add "$@" >/dev/null 2>&1
    if git -C "$tmp" commit -q -m "Test commit" >/dev/null 2>&1; then
        got=allowed
    else
        got=blocked
    fi
    if [ "$got" = "$want" ]; then
        echo "pass: $what ($got)"
        passed=$((passed + 1))
    else
        echo "FAIL: $what (expected $want, got $got)"
        failed=$((failed + 1))
    fi
    git -C "$tmp" reset -q >/dev/null 2>&1
    for last; do :; done
    rm -rf "${tmp:?}/$last"
}

# 1. A clean small file is allowed.
echo "fn main() {}" >"$tmp/clean.rs"
expect allowed "clean small file" clean.rs

# 2. A file larger than 50 MB is blocked.
head -c 52428801 /dev/zero >"$tmp/big.bin"
expect blocked "file larger than 50 MB" big.bin

# 3. A file of exactly 50 MB is allowed.
head -c 52428800 /dev/zero >"$tmp/limit.bin"
expect allowed "file of exactly 50 MB" limit.bin

# 4. A forbidden attribution string in file content is blocked.
trailer="Co-""Authored-""By"
printf 'Some text\n%s: Someone <someone@example.invalid>\n' "$trailer" >"$tmp/notes.md"
expect blocked "attribution trailer in file content" notes.md

# 5. An assistant name in file content is blocked.
name="Cla""ude"
printf '// Generated with %s\n' "$name" >"$tmp/gen.rs"
expect blocked "assistant name in file content" gen.rs

# 6. An assistant name in a file name is blocked.
fname="$(printf '%s' "$name" | tr 'A-Z' 'a-z')_notes.txt"
echo "text" >"$tmp/$fname"
expect blocked "assistant name in file name" "$fname"

# 7. A path under data/raw/ is blocked.
mkdir -p "$tmp/data/raw"
echo ">seq" >"$tmp/data/raw/genome.fa"
expect blocked "path under data/raw/" data

# 8. A path inside a work/ folder is blocked.
mkdir -p "$tmp/results/runs/r1/work"
echo "x" >"$tmp/results/runs/r1/work/chunk.bin"
expect blocked "path inside a work/ folder" results

# 9. A private key header is blocked.
b="-----""BEGIN"
printf '%s RSA PRIVATE KEY-----\nabc\n' "$b" >"$tmp/key.pem"
expect blocked "private key header" key.pem

# 10. A token-like string is blocked.
printf 'token = "gh''p_%s"\n' "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ" >"$tmp/config.toml"
expect blocked "access token" config.toml

# 11. A password in a config file is blocked.
printf 'pass''word = "hunter2hunter2"\n' >"$tmp/app.toml"
expect blocked "password in config" app.toml

# 12. A path listed in the local exclude file is blocked, even when forced.
echo "private.md" >>"$tmp/.git/info/exclude"
echo "private" >"$tmp/private.md"
expect blocked "path listed in .git/info/exclude" -f private.md

# 13. A commit message with an attribution trailer is blocked.
echo "fn f() {}" >"$tmp/msg.rs"
git -C "$tmp" add msg.rs
if git -C "$tmp" commit -q -m "Add f" -m "$trailer: Someone <someone@example.invalid>" >/dev/null 2>&1; then
    echo "FAIL: attribution trailer in commit message (expected blocked, got allowed)"
    failed=$((failed + 1))
else
    echo "pass: attribution trailer in commit message (blocked)"
    passed=$((passed + 1))
fi

# 14. A plain commit message is allowed.
if git -C "$tmp" commit -q -m "Add f" -m "What:" -m "- Add an empty function." >/dev/null 2>&1; then
    echo "pass: plain commit message (allowed)"
    passed=$((passed + 1))
else
    echo "FAIL: plain commit message (expected allowed, got blocked)"
    failed=$((failed + 1))
fi

echo ""
echo "Guard self-test: $passed passed, $failed failed."
[ "$failed" -eq 0 ]
