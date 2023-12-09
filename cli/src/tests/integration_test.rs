use std::fs;
use std::process::Command;

#[test]
fn test_prove_and_verify_recursive_proof_command() {
    // Path constants for the test
    const ELF_FILE: &str = "../examples/target/riscv32im-mozak-zkvm-elf/release/fibonacci";
    const IO_TAPE_PRIVATE: &str = "io_tape_private.txt";
    const IO_TAPE_PUBLIC: &str = "io_tape_public.txt";
    const PROOF_FILE: &str = "proof.bin";
    const RECURSIVE_PROOF_FILE: &str = "recursive_proof.bin";
    const RECURSIVE_PROOF_DB: &str = "recursive_proof.db";

    // Create mock IO tape files
    fs::write(IO_TAPE_PRIVATE, b"").expect("Failed to create IO tape private file");
    fs::write(IO_TAPE_PUBLIC, b"").expect("Failed to create IO tape public file");

    // Execute the `--prove` command using the 'fibonacci' ELF file
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "prove",
            ELF_FILE,
            IO_TAPE_PRIVATE,
            IO_TAPE_PUBLIC,
            PROOF_FILE,
            RECURSIVE_PROOF_FILE,
        ])
        .output()
        .expect("Failed to execute prove command");
    assert!(output.status.success(), "Prove command failed");

    // Assert the existence of output files
    for file in &[PROOF_FILE, RECURSIVE_PROOF_FILE, RECURSIVE_PROOF_DB] {
        let file_exists = fs::metadata(file).is_ok();
        assert!(file_exists, "Expected file {} not found", file);
    }

    // Execute the `--verify_recursive_proof` command
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "verify-recursive-proof",
            RECURSIVE_PROOF_FILE,
            RECURSIVE_PROOF_DB,
        ])
        .output()
        .expect("Failed to execute verify-recursive-proof command");
    assert!(
        output.status.success(),
        "Verify recursive proof command failed"
    );

    // Cleanup
    for file in &[
        IO_TAPE_PRIVATE,
        IO_TAPE_PUBLIC,
        PROOF_FILE,
        RECURSIVE_PROOF_FILE,
        RECURSIVE_PROOF_DB,
    ] {
        fs::remove_file(file).unwrap_or_else(|_| panic!("Failed to delete file {}", file));
    }
}
