use std::fs::{create_dir_all, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

const FIBONACCI_INPUT_CRATE: &str = "../examples/fibonacci-input";
const FIBONACCI_INPUT_ELF: &str =
    "../examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci-input";
const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn build_elf(crate_dir: &str, elf: &str, env: &str) {
    // Just touch for clippy
    if cfg!(feature = "cargo-clippy") {
        let elf = Path::new(FIBONACCI_INPUT_ELF);
        create_dir_all(elf.parent().unwrap()).expect("failed to touch elf dir");
        File::options()
            .create(true)
            .append(true)
            .open(elf)
            .expect("failed to touch elf");
    } else {
        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(crate_dir)
            .env_clear()
            .envs(std::env::vars().filter(|x| !x.0.starts_with("CARGO_")))
            .output()
            .expect("cargo command failed to run");
        if !output.status.success() {
            io::stdout().write_all(&output.stdout).unwrap();
            io::stderr().write_all(&output.stderr).unwrap();
            panic!("cargo build {crate_dir} failed.");
        }
    }

    println!("cargo:rerun-if-changed={crate_dir}");
    println!("cargo:rerun-if-changed={elf}");
    println!("cargo:rustc-env={env}={CARGO_MANIFEST_DIR}/{FIBONACCI_INPUT_ELF}");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    build_elf(
        FIBONACCI_INPUT_CRATE,
        FIBONACCI_INPUT_ELF,
        "FIBONACCI_INPUT_ELF",
    );
}
