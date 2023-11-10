#!/bin/sh

cd examples/fibonacci && cargo build --release && 
cd examples/fibonacci-input && cargo build --release
