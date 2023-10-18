use crate::id::Id;
use crate::Data;

/// A Program.
///
/// It contains the program code that is used to validate the object updates.
#[derive(Clone, Default)]
pub struct Program {
    /// Unique object ID
    pub id: Id,
    /// Program version (each update increases the version).
    /// TODO - explain how the version is used.
    pub version: u64,
    /// Flag if the program is mutable or not
    pub mutable: bool,
    /// Owner of the program. The owner describes how the program can change.
    pub owner: Id,
    /// Executable code for the program
    pub code: Data,
}

impl Program {
    /// Creates a new Program object.
    #[must_use]
    pub fn new(version: u64, mutable: bool, owner: Id, seed: u64, code: Data) -> Self {
        let id = Id::derive(owner, seed);
        Self {
            id,
            version,
            mutable,
            owner,
            code,
        }
    }
}
