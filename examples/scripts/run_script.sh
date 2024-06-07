#!/bin/bash

# script for mozakvm runner. this script would be run when 
# we invoke the command `cargo mozakvm-run`

set -euo pipefail

current_dir=$(pwd) 
project_root=$(git rev-parse --show-toplevel)
cd $project_root 
cargo build --bin run-example
cd $current_dir
$project_root/target/debug/run-example $@ -vvv --system-tape ../native/out/tape.json
