#!/bin/bash

set -euo pipefail

current_dir=$(pwd) 
cd ../../.. 
cargo build --bin mozak-cli 
project_root=$(pwd)
cd $current_dir
$project_root/target/debug/mozak-cli -vvv run $@
