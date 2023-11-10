#!/bin/sh

cd examples && cargo build --release --manifest-path fibonacci/Cargo.toml
&& cargo build --release --manifest-path fibonacci-input/Cargo.toml
