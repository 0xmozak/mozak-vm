use std::fs;
use std::path::{Path, PathBuf};

use log::{debug, trace};
use once_cell::sync::Lazy;
use regex::Regex;
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
    let file_bytes = fs::read_to_string(&cargo_toml_path).expect("Cargo.toml read failure");
    let parsed_toml: Value = file_bytes.parse().expect("Cargo.toml parsing error");
    let workspace_members = parsed_toml["workspace"]["members"]
        .as_array()
        .expect("Member finding error");
    workspace_members
        .into_iter()
        .map(|member| match member {
            Value::String(name) => WorkspaceMember {
                name: String::from(name),
                path: workspace_path.join(name),
            },
            _ => panic!("Cannot parse member as string: {:?}", member),
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

    let file_bytes = fs::read_to_string(&readme_path).expect("README.md read failure");
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
                .trim_start_matches("bash");

            // `step` is the string value `0-0` in a code block of following form:
            // ```sh
            // # inside examples directory
            // # [overseer/0-0]
            // cargo +nightly build --release --bin empty
            // ```
            let step = &string_repr.split("overseer/").skip(1).next().unwrap()[..3];

            let (major, minor) = (
                step[..1].parse::<usize>().unwrap(),
                step[2..].parse::<usize>().unwrap(),
            );

            if major > 0 {
                assert!(commands[major - 1].len() > 0);
            }
            if minor > 0 {
                assert!(commands[major].len() == minor);
            }

            commands[major].push(string_repr.into());
        });

    commands.retain(|inner_vec| !inner_vec.is_empty());

    if commands.len() > 0 {
        let mut debug_commands = String::new();
        for (major_idx, major_vec) in commands.iter().enumerate() {
            for (minor_idx, command) in major_vec.iter().enumerate() {
                debug_commands += format!(
                    "Step {}-{}: {}\n",
                    major_idx,
                    minor_idx,
                    command.replace("\n", "\n    ")
                )
                .as_str();
            }
        }
        debug!("Analysis of {:?}: \n{}", readme_path, debug_commands);
    }
    commands
}
