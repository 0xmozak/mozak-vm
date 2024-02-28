#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]

use std::path::Path;

use mozak_overseer::{extract_overseer_commandset, extract_workspace_members, WorkspaceMember};

fn main() {
    env_logger::init();

    for member in extract_workspace_members(Path::new("examples")) {
        run_overseer(&member);
    }
}

fn run_overseer(member: &WorkspaceMember) {
    // debug!("Running overseer for {:?}", member.path);
    extract_overseer_commandset(&member.path.join("README.md"));
}
