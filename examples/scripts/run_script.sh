#!/bin/bash
current_dir=$(pwd) &&
cd ../../.. &&
cargo build --release --bin mozak-cli &&
./target/release/mozak-cli -vvv run $@
