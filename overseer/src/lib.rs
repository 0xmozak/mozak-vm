use std::fs;
use std::path::{Path, PathBuf};

use log::trace;
use once_cell::sync::Lazy;
use regex::Regex;
use run_script::ScriptOptions;
use toml::Value;

#[derive(Default, Debug)]
pub struct WorkspaceMember {
    pub name: String,
    pub path: PathBuf,
}

/// Given `workspace_path`, extracts all the "members" found in
/// `workspace_path/Cargo.toml`
pub fn extract_workspace_members(workspace_path: &Path) -> Vec<WorkspaceMember> {
    let cargo_toml_path = workspace_path.join("Cargo.toml");
    let file_bytes = fs::read_to_string(cargo_toml_path).expect("Cargo.toml read failure");
    let parsed_toml: Value = file_bytes.parse().expect("Cargo.toml parsing error");
    let workspace_members = parsed_toml["workspace"]["members"]
        .as_array()
        .expect("Member finding error");
    workspace_members
        .iter()
        .map(|member| {
            if let Some(name) = member.as_str() {
                return WorkspaceMember {
                    name: String::from(name),
                    path: workspace_path.join(name),
                };
            }
            panic!("Cannot parse member as string: {:?}", member);
        })
        .collect()
}

/// Given a `README.md` file, extracts all code blocks under three backticks
/// with [overseer/num1/num2] comment in them. Returns a Vec of Vec sorted
/// first by num1, then by num2. Semantically, `[overseer/X/Y]` for some
/// constant `X` and `Y` going from `0` to some value `n` assumes commands
/// `[overseer/X/0]`, `[overseer/X/1]`, ..., `[overseer/X/n]` are supposed to
/// be executed in sequence with affected state shared between them. No state
/// is shared between `[overseer/A/...]` and `[overseer/B/...]` for different
/// `A` and `B`. This function panics if contigous sequence is not found, e.g.
/// `[overseer/0/2]` mandates some `[overseer/0/0]` and `[overseer/0/1]`.
pub fn extract_overseer_commandset(readme_path: &Path) -> Vec<Vec<String>> {
    trace!("Analysing README {:?}", readme_path);

    let file_bytes = fs::read_to_string(readme_path).expect("README.md read failure");
    static ALL_OVERSEER_CODE_BLOCK_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"```([\s\S]*?\[overseer/\d-\d\][\s\S]*?)```").expect("Invalid regex pattern")
    });

    let mut commands: Vec<Vec<String>> = vec![vec![]; 10];

    ALL_OVERSEER_CODE_BLOCK_REGEX
        .captures_iter(&file_bytes)
        .for_each(|capture| {
            let string_repr = capture
                .get(1)
                .unwrap()
                .as_str()
                .trim_start_matches("sh")
                .trim_start_matches("bash")
                .trim();

            // `step` is the string value `0-0` in a code block of following form:
            // ```sh
            // # inside examples directory
            // # [overseer/0-0]
            // cargo +nightly build --release --bin empty
            // ```
            let step = &string_repr.split("overseer/").nth(1).unwrap()[..3];

            let (major, minor) = (
                step[..1].parse::<usize>().unwrap(),
                step[2..].parse::<usize>().unwrap(),
            );

            if major > 0 {
                assert!(!commands[major - 1].is_empty());
            }
            if minor > 0 {
                assert!(commands[major].len() == minor);
            }

            commands[major].push(string_repr.into());
        });

    commands.retain(|inner_vec| !inner_vec.is_empty());

    if !commands.is_empty() {
        let mut debug_commands = String::new();
        for (major_idx, major_vec) in commands.iter().enumerate() {
            for (minor_idx, command) in major_vec.iter().enumerate() {
                debug_commands += format!(
                    "Step {}-{}: {}\n",
                    major_idx,
                    minor_idx,
                    command.replace('\n', "\n    ")
                )
                .as_str();
            }
        }
        trace!("Analysis of {:?}: \n{}", readme_path, debug_commands);
    }
    commands
}

pub fn clone_directory(src_path: &Path, dest_path: &Path) -> Result<(), std::io::Error> {
    // Create the destination directory if it doesn't exist
    setup_clean_dir(dest_path);

    // Iterate over the entries in the source directory
    for entry in fs::read_dir(src_path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dest_file_path = dest_path.join(entry.file_name());

        // Copy the file or directory to the destination
        if entry_path.is_dir() {
            // Recursively copy subdirectories
            clone_directory(&entry_path, &dest_file_path)?;
        } else {
            // Copy the file
            fs::copy(&entry_path, &dest_file_path)?;
        }
    }

    Ok(())
}

/// Removes any existing content at `path` and creates a clean directory.
/// May panic
#[allow(unused_must_use)]
pub fn setup_clean_dir(path: &Path) {
    fs::remove_dir_all(path);
    fs::create_dir_all(path).unwrap();
}

/// Runs a shell script with given `options` and yields a bool whether
/// execution ran correctly
pub fn run_shell_script(script: &Path, options: &ScriptOptions) -> bool {
    let script = fs::read_to_string(script).expect("script read failure");
    let child = run_script::spawn(script.as_str(), &vec![], options).unwrap();
    let spawn_output = child.wait_with_output().unwrap();
    spawn_output.status.success()
}
