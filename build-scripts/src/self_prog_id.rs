use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
pub const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
pub fn dump_self_prog_id(example: &str) -> Result<(), std::io::Error> {
    println!("cargo::rerun-if-changed=../mozakvm/main.rs");
    // build mozakvm binary
    let mozakvm_example_dir = Path::new("../mozakvm");
    let output = Command::new("cargo")
        .args(["mozakvm-build"])
        .current_dir(mozakvm_example_dir)
        .env_clear()
        .envs(std::env::vars().filter(|x| !x.0.starts_with("CARGO_")))
        .output()
        .expect("cargo command failed to run");
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
        panic!("cargo mozakvm-build failed.");
    }

    // use cli command to dump self_prog_id
    let target_path_str = format!(
        "../examples/{example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/{example}-mozakvm"
    );
    let cli_dir = Path::new(CARGO_MANIFEST_DIR).join("../cli");

    let mut output = Command::new("cargo")
        .args(vec![
            "run",
            "--bin",
            "dump-self-prog-id",
            "--",
            &target_path_str,
        ])
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

    // pop off the newline character
    assert_eq!(10, output.stdout.pop().unwrap());

    // store the self_prog_id in vars.rs
    let self_prog_id = String::from_utf8(output.stdout).unwrap();
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("vars.rs");
    let mut dest = File::create(dest_path).expect("failed to create vars.rs");
    writeln!(
        dest,
        r#"pub const {}_SELF_PROG_ID: &str =
               "{self_prog_id}";"#,
        example.to_ascii_uppercase()
    )
    .expect("can't write to vars.rs");
    Ok(())
}
