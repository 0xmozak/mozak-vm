use std::process::Command;
use std::fs;
use std::path::Path;

#[test]
fn test_prove_and_verify_recursive_proof_command() {
    // Path to the 'fibonacci' ELF file
    let elf_path = Path::new("src/tests/fibonacci");

    // Make sure the ELF file exists
    assert!(elf_path.exists(), "The 'fibonacci' ELF file does not exist in the tests directory");

    // Create mock IO tape files
    fs::write("io_tape_private.txt", b"").expect("Failed to create IO tape private file");
    fs::write("io_tape_public.txt", b"").expect("Failed to create IO tape public file");

    // Execute the `--prove` command using the 'fibonacci' ELF file
    let output = Command::new("cargo")
        .args(["run", "--", "prove", elf_path.to_str().unwrap(), "io_tape_private.txt", "io_tape_public.txt", "proof.bin", "recursive_proof.bin"])
        .output()
        .expect("Failed to execute prove command");
    assert!(output.status.success());

    assert!(fs::metadata("proof.bin").is_ok());
    assert!(fs::metadata("recursive_proof.bin").is_ok());
    assert!(fs::metadata("recursive_proof.db").is_ok());

    // Execute the `--verify_recursive_proof` command
    let output = Command::new("cargo")
        .args(["run", "--", "verify-recursive-proof", "recursive_proof.bin", "recursive_proof.db"])
        .output()
        .expect("Failed to execute verify-recursive-proof command");
    assert!(output.status.success());

    // Cleanup
    fs::remove_file("io_tape_private.txt").unwrap();
    fs::remove_file("io_tape_public.txt").unwrap();
    fs::remove_file("proof.bin").unwrap();
    fs::remove_file("recursive_proof.bin").unwrap();
    fs::remove_file("recursive_proof.db").unwrap();
}
