use std::process::Command;

// Example custom build script.
fn main() {
    // Tell Cargo that if the given file changes, to rerun this build script.
    // println!("cargo:rerun-if-changed=src/hello.c");
    // Use the `cc` crate to build a C file and statically link it.
    println!("cargo:rerun-if-changed=../Dockerfile");
    println!("cargo:rerun-if-changed=tests/testdata/.testdata_generated_from_this_commit");
    if !std::path::Path::new("tests/testdata/.testdata_generated_from_this_commit").exists() {
        assert!(Command::new("docker")
            .args(["buildx", "build", "--output", "tests/testdata", ".."])
            .status()
            .unwrap()
            .success())
        }
}
