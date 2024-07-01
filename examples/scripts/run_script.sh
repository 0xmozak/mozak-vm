#!/bin/bash

# script for mozakvm runner. this script would be run when
# we invoke the command `cargo mozakvm-run`

set -euo pipefail

mozakvm_dir=$(pwd)
project_root=$(git rev-parse --show-toplevel)
native_dir="$mozakvm_dir/../native"
system_tape_path="$native_dir/out/tape.json"
system_tape_arg=""

if [ -d "$native_dir" ]; then
    cd "$native_dir"
    cargo run
    system_tape_arg="--system-tape $system_tape_path"
    printf "\n Treating as fully featured example\n\n"
else
    printf "\n Native folder not found. Treating as stand-alone example\n\n"
fi

cd "$project_root"
cargo build --bin run-example

cd "$mozakvm_dir"
eval "$project_root/target/debug/run-example $* -vvv $system_tape_arg"
