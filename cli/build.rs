use std::process::Command;

const FIBONACCI_INPUT_CRATE: &str = "../examples/fibonacci-input";
const FIBONACCI_INPUT_ELF: &str =
    "../examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci-input";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={FIBONACCI_INPUT_CRATE}");
    println!("cargo:rerun-if-changed={FIBONACCI_INPUT_ELF}");

    Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(FIBONACCI_INPUT_CRATE)
        .spawn()
        .expect("ls command failed to start");

    println!(
        "cargo:rustc-env=FIBONACCI_INPUT_ELF={}/{FIBONACCI_INPUT_ELF}",
        env!("CARGO_MANIFEST_DIR")
    );
}
