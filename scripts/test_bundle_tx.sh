#!/bin/bash
# This script tests transaction bundling.

set -euo pipefail

root_dir=$(pwd)

token_dir="$root_dir/examples/token"
wallet_dir="$root_dir/examples/wallet"
example_dir="$root_dir/examples"

# Run native executions and build mozakvm binaries
cd "$token_dir/native" && cargo run --release
cd "$token_dir/mozakvm" && cargo mozakvm-build

cd "$wallet_dir/native" && cargo run --release
cd "$wallet_dir/mozakvm" && cargo mozakvm-build

# dump program ids
cd "$example_dir"
python "scripts/update_progams_map.py"

# Run CLI
cd "$root_dir"
cargo run --bin mozak-cli -- bundle-transaction -vvv \
    --system-tape examples/token/native/out/tape.json
