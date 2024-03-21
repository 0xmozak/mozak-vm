use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mozak_sdk::common::types::{ProgramIdentifier, SystemTape};
use mozak_sdk::native::ProofBundle;
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

    let elf_file: &str = "../examples/target/riscv32im-mozak-mozakvm-elf/release/fibonacci";

    // Create mock IO tape files
    fs::write(system_tape, b"").expect("Failed to create system tape file");

    // Execute the `prove` command
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "prove",
            elf_file,
            &proof_file.to_string_lossy(),
            "--self-prog-id",
            &format!("{:?}", &ProgramIdentifier::default()),
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
        ])
        .output()
        .expect("Failed to execute verify-recursive-proof command");
    assert!(
        output.status.success(),
        "Verify recursive proof command failed"
    );
}

#[test]
fn test_bundle_transaction_command() {
    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create a temporary directory");
    let temp_path = temp_dir.path();

    let prog_id = "MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538";

    let bundle_plans = temp_path.join("bundle.json");
    std::fs::create_dir_all(temp_path.join("examples/target/riscv32im-mozak-mozakvm-elf/release"))
        .unwrap();

    let system_tape_filepath = PathBuf::from("tests/token_tfr_bundle.json");
    let elf_filepath = temp_path.join("tokenbin");

    fs::write(&elf_filepath, b"").expect("Failed to create elf file");

    let cast_list = prog_id;
    let bundle = ProofBundle {
        self_prog_id: prog_id.to_string(),
        elf_filepath,
        system_tape_filepath,
    };

    // Create mock IO tape files
    fs::write(
        &bundle_plans,
        serde_json::to_string(&bundle).unwrap().as_bytes(),
    )
    .expect("Failed to create bundle file");

    // Execute the `prove` command
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "bundle-transaction",
            "--cast-list",
            cast_list,
            "--bundle-plans",
            &bundle_plans.to_string_lossy(),
        ])
        .output()
        .expect("Failed to execute prove command");
    assert!(
        output.status.success(),
        "Bundle transaction command failed: {:?}",
        output
    );
}
