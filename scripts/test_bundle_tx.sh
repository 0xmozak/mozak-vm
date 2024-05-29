#!/bin/bash
# This script tests transaction bundling.

set -euo pipefail

token_dir=examples/token
wallet_dir=examples/wallet

root_dir=$(pwd)
# Run native executions and build mozakvm binaries
cd "$token_dir/native" && cargo run --release
cd ../mozakvm && cargo build-mozakvm

cd $root_dir

cd examples/wallet/native && cargo run --release
cd ../mozakvm && cargo build-mozakvm

# Run CLI
cd $root_dir
cargo run --bin mozak-cli -- bundle-transaction -vvv \
    --system-tape examples/token/native/out/tape.json
