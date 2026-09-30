#!/bin/sh
# Converts projects saved before folder format 1.0 (format 8, `*.folder.ron`)
# to format 1.0 (`*.typr`), each beside its old file, which stays as it was.
# Format 8 holds the same as 1.0: only the version is written differently.
#
#   scripts/convert-format-8.sh novel.folder.ron [more.folder.ron ...]
#
# Drafts are in ~/.local/share/typewriter/drafts. Files from before format 8
# need saving once with a build from before 1.0 first.
set -eu

if [ $# -eq 0 ]; then
    echo "usage: $0 project.folder.ron [...]" >&2
    exit 2
fi

status=0
for old in "$@"; do
    new="${old%.folder.ron}.typr"
    if [ ! -f "$old" ]; then
        echo "$old: no such file" >&2
        status=1
        continue
    fi
    # The opening `(version: N,` alone: typed text further on is left be.
    format=$(head -c 200 "$old" | tr -d ' \t\r\n' | sed -n 's/^(version:\([0-9]*\),.*/\1/p')
    if [ "$format" != 8 ]; then
        echo "$old: not a format 8 project (${format:-no format number}): left as it is" >&2
        status=1
        continue
    fi
    if [ -e "$new" ]; then
        echo "$new already exists: $old left as it is" >&2
        status=1
        continue
    fi
    awk '!done && /version: *8,/ { sub(/version: *8,/, "version: \"1.0\","); done = 1 } { print }' \
        "$old" >"$new"
    echo "$old -> $new"
done
exit $status
