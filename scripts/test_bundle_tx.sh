#!/bin/sh
# This script tests transaction bundling.

# Run native execution and build mozakvm binary
cd examples/token && cargo run --release \
    --features="native" \
    --bin token-native \
    --target "$(rustc -vV | grep host | awk '{ print $2; }')" \
    && cargo build --bin tokenbin --release

# Run CLI
cd ../../
cargo run --bin mozak-cli -- bundle-transaction -vvv \
    --system-tape examples/token/out/token.tape.json
