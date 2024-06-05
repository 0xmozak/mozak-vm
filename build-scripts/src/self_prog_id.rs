use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
pub const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
pub fn dump_self_prog_id(example: &str) -> Result<(), std::io::Error> {
    // build mozakvm binary
    let mozakvm_example_dir = Path::new("../mozakvm");
    let output = Command::new("cargo")
        .args(["build-mozakvm"])
        .current_dir(mozakvm_example_dir)
        .env_clear()
        .envs(std::env::vars().filter(|x| !x.0.starts_with("CARGO_")))
        .output()
        .expect("cargo command failed to run");
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("cargo build-mozakvm failed.");
    }

    // use cli command to dump self_prog_id
    let target_path_str = format!(
        "../examples/{example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/{example}-mozakvm"
    );
    let cli_dir = Path::new(CARGO_MANIFEST_DIR).join("../cli");

    let output = Command::new("cargo")
        .args(vec!["run", "--", "self-prog-id", &target_path_str])
        .current_dir(cli_dir)
        .env_clear()
        .envs(std::env::vars().filter(|x| !x.0.starts_with("CARGO_")))
        .output()
        .expect("mozak-cli's command self-prog-id failed");
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("mozak-cli's command self-prog-id failed");
    }

    let mut self_prog_id_file = File::create("self_prog_id.txt")?;
    self_prog_id_file.write_all(&output.stdout)
}
