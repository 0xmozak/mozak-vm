#!/bin/bash

# script for mozakvm runner. this script would be run when 
# we invoke the command `cargo run-mozakvm`

set -euo pipefail

current_dir=$(pwd) 
cd ../../..
cargo build --bin run-example
project_root=$(pwd)
cd $current_dir
$project_root/target/debug/run-example $@ -vvv --system-tape ../native/out/tape.json
