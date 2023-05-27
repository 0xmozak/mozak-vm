use std::process::Command;

fn main() {
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
