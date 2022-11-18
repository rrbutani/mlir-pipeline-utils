#!/usr/bin/env bash

set -eu -o pipefail

readonly OUT="${1-"dump"}"
mkdir -p "$OUT"

echo -ne "Waiting for input from stdin..\r" >&2
declare -i _num=0

# IFS="" to preserve leading whitespace..
while IFS="" read -r _line; do
    :
done


# TODO: bash adds something like 300x overhead compared to just `cat`ing the
# input, without even doing any processing; eventually we should rewrite this in
# an actual language..
