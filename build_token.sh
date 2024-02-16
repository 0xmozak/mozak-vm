cd examples && cargo build --bin tokenbin &&
../target/debug/mozak-cli prove-and-verify -vvv target/riscv32im-mozak-zkvm-elf/debug/tokenbin --system-tape wallet_tfr.tape_bin
