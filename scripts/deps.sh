#!/usr/bin/env bash
# Generates a dependency graph _for this repository only_, in mermaid format.

echo 'graph TD;'

for x in "$(dirname "$0")"/../src/*/Cargo.toml
do
    pkg_name="$(basename "$(dirname "$x")")"
    for dep in $(cat "$x" | grep -oE '^many-?[^ ]*')
    do
        echo "  $pkg_name --> $dep;"
    done

done
