cd examples && cargo build --release --bin tokenbin &&
MOZAK_STARK_DEBUG=true ../target/debug/mozak-cli prove-and-verify -vvv target/riscv32im-mozak-zkvm-elf/release/tokenbin --system-tape wallet_tfr.tape_bin
