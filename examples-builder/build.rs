use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

struct Crate {
    crate_path: &'static str,
    elf_path: &'static str,
    glob_name: &'static str,
    enabled: bool,
    uses_std: bool,
}

macro_rules! ecrate {
    ($name:literal, $glob:literal, $uses_std:expr) => {
        ecrate!($name, $name, $glob, $uses_std)
    };
    ($name:literal, $file:literal, $glob:literal, $uses_std:expr) => {
        Crate {
            crate_path: concat!("../examples/", $name, "/mozakvm"),
            elf_path: concat!(
                "../examples/",
                $name,
                "/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/",
                $file,
                "-mozakvm"
            ),
            glob_name: $glob,
            enabled: cfg!(feature = $name),
            uses_std: $uses_std,
        }
    };
}

const CRATES: &[Crate] = &[
    ecrate!("bss-tester", "BSS_ELF", false),
    ecrate!("fibonacci", "FIBONACCI_ELF", false),
    ecrate!("memory-access", "MEMORY_ACCESS_ELF", false),
    ecrate!("min-max", "MIN_MAX_ELF", false),
    ecrate!("panic", "PANIC_ELF", false),
    ecrate!("rkyv-serialization", "RKYV_SERIALIZATION_ELF", false),
    ecrate!("sha2", "SHA2_ELF", false),
    ecrate!("static-mem-access", "STATIC_MEM_ACCESS_ELF", false),
    ecrate!("empty", "EMPTY_ELF", false),
    ecrate!("mozak-sort", "MOZAK_SORT_ELF", false),
    ecrate!("token", "TOKENBIN", false),
    ecrate!("wallet", "WALLETBIN", false),
    ecrate!("inputtape", "INPUTTAPEBIN", false),
];
const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn build_elf(dest: &mut File, crate_path: &str, elf_path: &str, glob_name: &str, uses_std: bool) {
    // Use a dummy array for clippy, since not building the elf is faster than
    // building the elf
    if cfg!(feature = "cargo-clippy") {
        writeln!(dest, r#"pub const {glob_name}: &[u8] = &[];"#)
    } else {
        let args = if uses_std {
            vec!["build-mozakvm", "--features=std"]
        } else {
            vec!["build-mozakvm"]
        };
        let output = Command::new("cargo")
            .args(args)
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
        writeln!(
            dest,
            r#"pub const {glob_name}: &[u8] =
                   include_bytes!(r"{CARGO_MANIFEST_DIR}/{elf_path}");"#
        )
    }
    .expect("failed to write vars.rs");

    println!("cargo:rerun-if-changed={crate_path}");
    println!("cargo:rerun-if-changed={elf_path}");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("vars.rs");
    let mut dest = File::create(dest_path).expect("failed to create vars.rs");

    for c in CRATES {
        if c.enabled {
            build_elf(&mut dest, c.crate_path, c.elf_path, c.glob_name, c.uses_std)
        }
    }
}
