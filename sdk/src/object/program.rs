extern crate alloc;
use alloc::vec::Vec;

use super::*;
use crate::Transition;

/// A Program type of object.
///
/// This object type is used to constrain the
/// evolution of other objects. It contains the program code that is used to
/// validate the object evolution.
#[derive(Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ProgramContent {
    /// Unique object ID
    id: Id,
    /// Program version (each update increases the version).
    /// TODO - explain how the version is used.
    version: u64,
    /// Flag if the program is mutable or not
    mutable: bool,
    /// Owner of the program. The owner describes how the program can change.
    owner: Id,
    /// A list of accepted transitions that are allowed to be executed on
    /// the objects that are owned.
    /// During state update the user will propose a new state,
    /// as well as provide the ID of the transition that validates
    /// the state update.
    /// The transition ID is a hash of the program that is used to validate
    /// the transition.
    pub validating_transitions: Vec<Transition>,
}

impl ObjectContent for ProgramContent {
    fn id(&self) -> Id { self.id }

    fn owner(&self) -> &Id { &self.owner }
}

#[cfg(feature = "std")]
impl ProgramContent {
    /// Creates a new Program object.
    pub fn new(
        version: u64,
        mutable: bool,
        owner: Id,
        validating_transitions: Vec<Transition>,
    ) -> Self {
        let id = Self::generate_id(vec![
            version.to_be_bytes().to_vec(),
            vec![mutable as u8],
            owner.to_vec(),
            validating_transitions
                .iter()
                .flat_map(|t| t.id().to_vec())
                .collect(),
        ]);

        Self {
            id,
            version,
            mutable,
            owner,
            validating_transitions,
        }
    }
}
