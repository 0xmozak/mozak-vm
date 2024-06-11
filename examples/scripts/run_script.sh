#!/bin/bash

# script for mozakvm runner. this script would be run when
# we invoke the command `cargo mozakvm-run`

set -euo pipefail

mozakvm_dir=$(pwd)
project_root=$(git rev-parse --show-toplevel)
system_tape_path="../native/out/tape.json"
system_tape_arg=""

if [ -f $system_tape_path ]; then
    system_tape_arg="--system-tape $system_tape_path"
else
    printf "\nsystem tape not found. Treating as standalone example\n\n"
fi

cd "$project_root"
cargo build --bin run-example
cd "$mozakvm_dir"
eval "$project_root/target/debug/run-example $* -vvv $system_tape_arg"
