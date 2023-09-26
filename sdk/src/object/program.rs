use std::collections::HashMap;

use super::*;
use crate::TransitionFunction;

/// A Program type of object.
///
/// This object type is used to constrain the
/// evolution of other objects. It contains the program code that is used to
/// validate the object evolution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProgramContent {
    /// Unique object ID
    id: Id,
    /// Program version (each update increases the version).
    /// TODO - explain how the version is used.
    version: u64,
    /// Flag if the program is mutable or not
    mutable: bool,
    /// Owner of the program. The owner describes how the program can
    /// evolve.
    owner_id: Id,
    /// A list of accepted transitions that are allowed to be executed on
    /// the objects that are owned.
    /// During state update the user will propose a new state,
    /// as well as provide the ID of the transition that validates
    /// the state update.
    /// The transition ID is a hash of the program that is used to validate
    /// the transition.
    pub allowed_transitions: HashMap<Id, TransitionFunction>,
}

impl ObjectContent for ProgramContent {
    fn id(&self) -> Id { self.id }

    fn owner_id(&self) -> &Id { &self.owner_id }
}

impl ProgramContent {
    /// Creates a new Program object.
    pub fn new(
        version: u64,
        mutable: bool,
        owner_id: Id,
        validating_transitions: Vec<TransitionFunction>,
    ) -> Self {
        let id = Self::generate_id(vec![
            version.to_be_bytes().to_vec(),
            vec![mutable as u8],
            owner_id.to_vec(),
            validating_transitions
                .iter()
                .flat_map(transition_to_bytes)
                .collect(),
        ]);

        let validating_transitions = validating_transitions
            .into_iter()
            .map(|transition| {
                let id = generate_transition_id(&transition);
                (id, transition)
            })
            .collect();

        Self {
            id,
            version,
            mutable,
            owner_id,
            allowed_transitions: validating_transitions,
        }
    }
}

/// Converts a transition function into a byte vector.
/// TODO - add code that converts the transition into bytes.
fn transition_to_bytes(transition: &TransitionFunction) -> Vec<u8> {
    let mut result = vec![];

    result.extend_from_slice(&transition.entry_point.to_be_bytes());

    result
}

/// Generates a unique ID for the transition function.
/// Currently, we use SHA3-256 hash function to generate the ID.
pub fn generate_transition_id(transition: &TransitionFunction) -> Id {
    let mut hasher = sha3::Sha3_256::new();
    hasher.update(transition_to_bytes(transition));
    let hash = hasher.finalize();

    Id(hash.into())
}
