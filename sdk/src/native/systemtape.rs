use std::fs;

/// Writes a byte slice to a given file
fn write_to_file(file_path: &str, content: &[u8]) {
    use std::io::Write;
    let path = std::path::Path::new(file_path);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(content).unwrap();
}

/// Dumps a copy of `SYSTEM_TAPE` to disk, serialized
/// via `serde_json` as well as in rust debug file format
/// if opted for. Extension of `.tape.json` is used for serialized
/// formed of tape on disk, `.tape.debug` will be used for
/// debug tape on disk.
#[allow(dead_code)]
fn dump_system_tape(is_debug_tape_required: bool) {
    fs::create_dir_all("out").unwrap();
    let tape_clone = unsafe {
        crate::common::system::SYSTEM_TAPE.clone() // .clone() removes `Lazy{}`
    };

    if is_debug_tape_required {
        write_to_file("out/tape.debug", &format!("{tape_clone:#?}").into_bytes());
    }

    write_to_file(
        "out/tape.json",
        &serde_json::to_string_pretty(&tape_clone)
            .unwrap()
            .into_bytes(),
    );
}

/// This functions dumps 2 files of the currently running guest program:
///   1. the actual system tape (JSON),
///   2. the debug dump of the system tape,
///
/// These are all dumped in a sub-directory named `out` in the project root. The
/// user must be cautious to not move at least the system tape, as the system
/// tape is used by the CLI in proving and in transaction bundling, and the SDK
/// makes some assumptions about where to find the ELF for proving.
pub fn dump_proving_files() { dump_system_tape(true); }
