#!/usr/bin/env bash
# Generates a dependency graph _for this repository only_, in mermaid format.

echo 'graph TD;'

for x in "$(dirname "$0")"/../src/*/Cargo.toml
do
    pkg_name="$(basename "$(dirname "$x")")"

    deps="$(grep -oE '^many-?[^ ]*' "$x" | tr '\n' '&' | sed 's/&$//' | sed 's/&/ & /g')"
    [ "$deps" ] && echo "  $pkg_name --> $deps;"
done
