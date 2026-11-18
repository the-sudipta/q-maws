#!/bin/bash
# Runs a command, shows its output, and when it fails publishes the last 60 lines as one
# error annotation of the job. Annotations of a public repository can be read without
# signing in (GitHub API: check-runs/<job>/annotations), the job logs cannot.
#
# Usage: bash scripts/cloud/logged.sh <title> <command> [arguments...]

title=${1:?title}; shift
log=$(mktemp)
"$@" 2>&1 | tee "$log"
rc=${PIPESTATUS[0]}
if [ "$rc" -ne 0 ]; then
    # one annotation: newlines and % encoded as GitHub expects
    msg=$(tail -n 60 "$log" | tr -d '\r' | sed -e 's/%/%25/g' | awk 'BEGIN{ORS="%0A"} {print}')
    echo "::error title=$title (exit $rc)::$msg"
fi
rm -f "$log"
exit "$rc"
