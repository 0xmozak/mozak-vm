use std::fs;
use std::process::Command;
use std::time::Instant;
use std::str;

use tempfile::TempDir;

#[test]
fn test_prove_and_verify_recursive_proof_command() {
    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create a temporary directory");
    let temp_path = temp_dir.path();

    // Define file paths inside the temporary directory
    let io_tape_private = temp_path.join("io_tape_private.txt");
    let io_tape_public = temp_path.join("io_tape_public.txt");
    let proof_file = temp_path.join("proof.bin");
    let recursive_proof_file = temp_path.join("recursive_proof.bin");
    let recursive_proof_db = temp_path.join("recursive_proof.db");

    let elf_file: &str = "../examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci";

    // Create mock IO tape files
    fs::write(&io_tape_private, b"").expect("Failed to create IO tape private file");
    fs::write(&io_tape_public, b"").expect("Failed to create IO tape public file");

    // Start timer for prove command
    let start = Instant::now();

    // Execute the `--prove` command
    let output = Command::new("cargo")
        .args([
            "run",
            "-r",
            "--",
            "prove",
            elf_file,
            &proof_file.to_string_lossy(),
            &io_tape_private.to_string_lossy(),
            &io_tape_public.to_string_lossy(),
            &recursive_proof_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to execute prove command");
    assert!(output.status.success(), "Prove command failed");

    // Stop timer and print duration for prove command
    let duration = start.elapsed();
    println!("Time taken for prove command: {:?}", duration);

    // Print stdout and stderr
    if !output.stdout.is_empty() {
        println!("Standard Output:\n{}", str::from_utf8(&output.stdout).unwrap_or("[Invalid UTF-8 in stdout]"));
    }
    if !output.stderr.is_empty() {
        println!("Standard Error:\n{}", str::from_utf8(&output.stderr).unwrap_or("[Invalid UTF-8 in stderr]"));
    }

    // Assert the existence of output files
    for file in &[&proof_file, &recursive_proof_file, &recursive_proof_db] {
        let file_exists = file.exists();
        assert!(file_exists, "Expected file {:?} not found", file);
    }

    // Start timer for verify-recursive-proof command
    let start = Instant::now();

    // Execute the `--verify_recursive_proof` command
    let output = Command::new("cargo")
        .args([
            "run",
            "-r",
            "--",
            "verify-recursive-proof",
            &recursive_proof_file.to_string_lossy(),
            &recursive_proof_db.to_string_lossy(),
        ])
        .output()
        .expect("Failed to execute verify-recursive-proof command");
    assert!(
        output.status.success(),
        "Verify recursive proof command failed"
    );

    // Stop timer and print duration for verify-recursive-proof command
    let duration = start.elapsed();
    println!(
        "Time taken for verify-recursive-proof command: {:?}",
        duration
    );

    // Print stdout and stderr
    if !output.stdout.is_empty() {
        println!("Standard Output:\n{}", str::from_utf8(&output.stdout).unwrap_or("[Invalid UTF-8 in stdout]"));
    }
    if !output.stderr.is_empty() {
        println!("Standard Error:\n{}", str::from_utf8(&output.stderr).unwrap_or("[Invalid UTF-8 in stderr]"));
    }
}
