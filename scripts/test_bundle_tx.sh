#!/bin/bash
# This script tests transaction bundling.

set -euo pipefail

# Run native executions and build mozakvm binaries
cd examples/token/native && cargo run --release &&
    # --features="native" \
    # --bin token-native \
    # --target "$(rustc --verbose --version | grep host | awk '{ print $2; }')" &&
    cd ../mozakvm &&
    cargo build-mozakvm
cd examples/wallet/native && cargo run --release &&
    # --features="native" \
    # --bin token-native \
    # --target "$(rustc --verbose --version | grep host | awk '{ print $2; }')" &&
    cd ../mozakvm &&
    cargo build-mozakvm

# Run CLI
cd ../../../
cargo run --bin mozak-cli -- bundle-transaction -vvv \
    --system-tape examples/token/native/out/tape.json
