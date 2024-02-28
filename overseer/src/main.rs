use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use log::{debug, info, trace};
use mozak_overseer::{
    clone_directory, extract_overseer_commandset, extract_workspace_members, run_shell_script,
    setup_clean_dir,
};
use run_script::ScriptOptions;

fn main() {
    env_logger::init();
    run_overseer(Path::new("examples"));
}

#[allow(unused_must_use)]
fn run_overseer(dir: &Path) {
    let overseer_dir = dir.join(".overseer");

    // Setup
    setup_clean_dir(&overseer_dir);
    clone_directory(&dir.join(".cargo"), &overseer_dir.join(".cargo")).unwrap();
    fs::remove_dir_all(dir.join("target"));

    let mut executions: HashMap<String, Vec<PathBuf>> = HashMap::new();

    // Commands builder
    // Builds `.overseer/scratch/...` for all workspace members
    // Done explicitly before any execution to aid in verification if needed
    for member in extract_workspace_members(Path::new("examples")) {
        let commands = extract_overseer_commandset(&member.path.join("README.md"));
        let member_scratch_dir = &overseer_dir.join("scratch").join(&member.name);

        for (major_idx, major_vec) in commands.iter().enumerate() {
            let commands_dir = &member_scratch_dir.join(format!("{}", major_idx));
            setup_clean_dir(commands_dir);
            let mut paths: Vec<PathBuf> = Vec::with_capacity(major_vec.len());
            for (minor_idx, command) in major_vec.iter().enumerate() {
                let script_path =
                    commands_dir.join(format!("script-{}-{}.sh", major_idx, minor_idx));
                paths.push(script_path.clone());
                fs::write(&script_path, command).unwrap()
            }
            let key = format!("{}/{}/{}", dir.display(), member.name, major_idx);
            debug!("Adding {} with {} steps", key, paths.len());
            executions.insert(key, paths);
        }
    }

    // Commands execution
    for (id, scripts) in executions.iter() {
        debug!("Executing {}", id);

        // Setup
        clone_directory(&overseer_dir.join(".cargo"), &dir.join(".cargo")).unwrap();
        setup_clean_dir(&dir.join("target"));

        // Execute
        for script in scripts {
            trace!("Executing shell script: {:?}", script);
            assert!(run_shell_script(script, &ScriptOptions {
                runner: None,
                runner_args: None,
                working_directory: Some(dir.into()),
                input_redirection: run_script::types::IoOptions::Inherit,
                output_redirection: run_script::types::IoOptions::Inherit,
                print_commands: true,
                exit_on_error: false,
                env_vars: Some(HashMap::<String, String>::from([(
                    String::from("MOZAK_DEBUG"),
                    String::from("true")
                )]))
            }));
        }
    }

    // Cleanup, may fail if execution fails
    // Setup needs to ensure cleanliness
    // Commented since locally you may want to have a look into
    // `.overseer` post run.

    // setup_clean_dir(&overseer_dir);

    info!("All mozak overseer tests passed successfully!");
}
