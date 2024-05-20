use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Command;

const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
/// dumps the `ProgramIdentifier` of the input ELF
/// in a file `self_prog_id.txt` lying in same directory.
/// Note that the `ProgramIdentifier` is represented as
/// a string, for example,
/// `MZK-8279ede6d0459962efe306b82d6036c08d6b9e66ad8d89d9e297c5222b3f4572`
pub fn dump_self_prog_id(out_elf_name: &str) {
    let cargo_manifest_dir_path = Path::new(CARGO_MANIFEST_DIR);
    let out_dir =
        cargo_manifest_dir_path.join("../examples/target/riscv32im-mozak-mozakvm-elf/release/");
    let out_elf_path = out_dir.join(out_elf_name);
    let out_elf_path_str = out_elf_path.to_str().unwrap();
    let cli_dir = cargo_manifest_dir_path.join("../cli");

    let args = vec!["self-prog-id", out_elf_path_str];

    // execute the cli command `self-prog-id` on the ELF
    let output = Command::new("./../target/release/mozak-cli")
        .args(args)
        .current_dir(cli_dir)
        .env_clear()
        .envs(std::env::vars().filter(|x| !x.0.starts_with("CARGO_")))
        .output()
        .expect("can't find mozak-cli. Please run cargo build --release --bin mozak-cli from project root");

    if !output.status.success() {
        io::stdout().write_all(&output.stdout).unwrap();
        io::stderr().write_all(&output.stderr).unwrap();
        panic!("cargo run -- self-prog-id {out_elf_path_str} failed");
    }

    let self_prog_id = String::from_utf8(output.stdout).unwrap();
    let self_prog_id = self_prog_id.trim();

    // write `self_prog_id` to a file
    let mut self_prog_id_file =
        File::create("self_prog_id.txt").expect("failed to create self_prog_id.txt");
    write!(self_prog_id_file, "{}", self_prog_id).expect("failed to write self_prog_id.txt");
    println!("cargo:rerun-if-changed={}", out_elf_path_str);
}
