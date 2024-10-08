use std::fs;
use std::process::Command;

use tempfile::TempDir;

#[test]
fn test_prove_and_verify_recursive_proof_command() {
    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create a temporary directory");
    let temp_path = temp_dir.path();

    // Define file paths inside the temporary directory
    let system_tape = temp_path.join("system_tape.txt");
    let proof_file = temp_path.join("proof.bin");
    let recursive_proof_file = temp_path.join("recursive_proof.bin");
    let recursive_proof_vk = temp_path.join("recursive_proof.vk");

    let elf_file: &str =
        "../examples/fibonacci/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/fibonacci-mozakvm";

    // Create mock IO tape files
    fs::write(system_tape, b"").expect("Failed to create system tape file");

    // Get self_prog_id
    let output = Command::new("cargo")
        .args(["run", "--", "self-prog-id", elf_file])
        .output()
        .expect("Failed to execute self-prog-id command");
    let mut self_prog_id = String::from_utf8(output.stdout).unwrap();
    self_prog_id = self_prog_id.trim().to_string();

    // Execute the `prove` command
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "prove",
            elf_file,
            &proof_file.to_string_lossy(),
            &recursive_proof_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to execute prove command");
    assert!(
        output.status.success(),
        "Prove command failed: {:?}",
        output
    );

    // Assert the existence of output files
    for file in &[&proof_file, &recursive_proof_file, &recursive_proof_vk] {
        let file_exists = file.exists();
        assert!(file_exists, "Expected file {:?} not found", file);
    }

    // Execute the `--verify_recursive_proof` command
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "verify-recursive-proof",
            &recursive_proof_file.to_string_lossy(),
            &recursive_proof_vk.to_string_lossy(),
            &self_prog_id,
        ])
        .output()
        .expect("Failed to execute verify-recursive-proof command");
    assert!(
        output.status.success(),
        "Verify recursive proof command failed"
    );
}
