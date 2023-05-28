use std::process::Command;

fn main() {
    let stamp_actual = "tests/testdata/.testdata_generated_from_this_commit";
    let stamp_wanted = "tests/create_testdata/generate_testdata_from_this_commit";

    println!("cargo:rerun-if-changed=../Dockerfile");
    println!("cargo:rerun-if-changed={}", stamp_actual);
    println!("cargo:rerun-if-changed={}", stamp_wanted);

    if std::fs::read_to_string(stamp_wanted).unwrap()
        != std::fs::read_to_string(stamp_actual).unwrap_or_default()
    {
        assert!(Command::new("docker")
            .args([
                "buildx",
                "build",
                "--output",
                "tests/testdata",
                "tests/create_testdata/"
            ])
            .status()
            .unwrap()
            .success())
    }
}
