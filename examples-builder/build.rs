use std::fs::{create_dir_all, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

struct Crate {
    crate_path: &'static str,
    elf_path: &'static str,
    glob_name: &'static str,
    enabled: bool,
}

macro_rules! ecrate {
    ($name:literal, $glob:literal) => {
        Crate {
            crate_path: concat!("../examples/", $name),
            elf_path: concat!(
                "../examples/target/riscv32im-mozak-zkvm-elf/release/",
                $name
            ),
            glob_name: $glob,
            enabled: cfg!(feature = $name),
        }
    };
}

const CRATES: &[Crate] = &[
    ecrate!("fibonacci", "FIBONACCI_ELF"),
    ecrate!("fibonacci-input", "FIBONACCI_INPUT_ELF"),
    ecrate!("memory-access", "MEMORY_ACCESS_ELF"),
    ecrate!("min-max", "MIN_MAX_ELF"),
    ecrate!("panic", "PANIC_ELF"),
    ecrate!("poseidon2", "POSEIDON2_ELF"),
    ecrate!("rkyv-serialization", "RKYV_SERIALIZATION_ELF"),
    ecrate!("sha2", "SHA2_ELF"),
    ecrate!("static-mem-access", "STATIC_MEM_ACCESS_ELF"),
    ecrate!("stdin", "STDIN_ELF"),
];
const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn build_elf(dest: &mut File, crate_path: &str, elf_path: &str, glob_name: &str) {
    // Just touch for clippy
    if cfg!(feature = "cargo-clippy") {
        let elf_path = Path::new(elf_path);
        create_dir_all(elf_path.parent().unwrap()).expect("failed to touch elf dir");
        File::options()
            .create(true)
            .append(true)
            .open(elf_path)
            .expect("failed to touch elf");
    } else {
        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(crate_path)
            .env_clear()
            .envs(std::env::vars().filter(|x| !x.0.starts_with("CARGO_")))
            .output()
            .expect("cargo command failed to run");
        if !output.status.success() {
            io::stdout().write_all(&output.stdout).unwrap();
            io::stderr().write_all(&output.stderr).unwrap();
            panic!("cargo build {crate_path} failed.");
        }
    }

    println!("cargo:rerun-if-changed={crate_path}");
    println!("cargo:rerun-if-changed={elf_path}");

    writeln!(
        dest,
        r#"pub const {glob_name}: &[u8] = include_bytes!("{CARGO_MANIFEST_DIR}/{elf_path}");"#
    )
    .expect("failed to write vars.rs");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("vars.rs");
    let mut dest = File::create(dest_path).expect("failed to create vars.rs");

    for c in CRATES {
        if c.enabled {
            build_elf(&mut dest, c.crate_path, c.elf_path, c.glob_name)
        }
    }
}
