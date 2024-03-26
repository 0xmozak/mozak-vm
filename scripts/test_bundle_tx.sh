#!/bin/sh
# This script tests transaction bundling.

# Run natives and build mozakvm binaries
cd examples/token && cargo run --release --features="native" --bin token-native --target aarch64-apple-darwin && cargo build --bin tokenbin
cd ../wallet && cargo run --release --features="native" --bin wallet-native --target aarch64-apple-darwin && cargo build --bin walletbin

# Run CLI
cd ../../
cargo run --bin mozak-cli -- bundle-transaction \
    --cast-list MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538 \
    --cast-list MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb \
    --bundle-plan examples/token/out/token_tfr_bundle.json \
    --bundle-plan examples/wallet/out/wallet_approve_bundle.json