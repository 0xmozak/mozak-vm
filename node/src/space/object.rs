use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

pub use mozak_vm::elf::Program as Transition;
use serde::{Deserialize, Serialize};
use sha3::digest::FixedOutput;
use sha3::Digest;

use crate::Id;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Object {
    Program(program::ProgramContent),
    Data(data::DataContent),
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Object::Program(a), Object::Program(b)) => a.id() == b.id(),
            (Object::Data(a), Object::Data(b)) => a.id() == b.id(),
            _ => false,
        }
    }
}

impl Object {
    pub(crate) fn id(&self) -> Id {
        match self {
            Object::Program(program) => program.id(),
            Object::Data(data) => data.id(),
        }
    }

    pub fn as_program(&self) -> Option<&program::ProgramContent> {
        match self {
            Object::Program(program) => Some(program),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<&data::DataContent> {
        match self {
            Object::Data(data) => Some(data),
            _ => None,
        }
    }
}

/// Wraps an Object Content into an Object enum.
/// ProgramContent -> Object::Program
/// DataContent -> Object::Data
macro_rules! build_wrapper {
    ($content:ty, $variant:ident) => {
        impl From<$content> for Object {
            fn from(content: $content) -> Self { Object::$variant(content) }
        }
    };
}

build_wrapper!(program::ProgramContent, Program);
build_wrapper!(data::DataContent, Data);

impl Eq for Object {}

/// Our application space is a collection of objects of different types, which
/// depends on the type of data that is stored in the object.
pub(crate) trait ObjectContent: Debug + Clone {
    /// The unique id of the object, can be considered as an object address.
    ///
    /// It is generated deterministically based on the constraining program id,
    /// and some additional parameters that are provided during the object
    /// creation.
    fn id(&self) -> Id;

    /// Id of an object that controls how the object can evolve.
    /// Typically, this object would be a program that has a list of state
    /// transitions that are allowed.
    fn owner_id(&self) -> &Id;

    /// The data that is stored in the object. This function is agnostic to the
    /// type of the object
    fn data(&self) -> Data;

    /// Generates a unique ID for the object.
    /// Currently, we use SHA3-256 hash function to generate the ID.
    fn generate_id<T: Into<Box<[u8]>> + Clone>(parameters: Vec<T>) -> Id {
        let mut hasher = sha3::Sha3_256::new();
        parameters
            .iter()
            .for_each(|p| hasher.update(p.clone().into()));
        let hash = hasher.finalize();

        Id(hash.into())
    }
}

/// Generic data representation, that all objects should be able to convert to.
type Data = Vec<u8>;

pub(crate) mod program {
    use std::collections::HashMap;

    use super::*;

    /// A Program type of object.
    ///
    /// This object type is used to constrain the
    /// evolution of other objects. It contains the program code that is used to
    /// validate the object evolution.
    #[derive(Debug, Clone, Serialize, Deserialize)]
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
        pub accepted_transitions: HashMap<Id, Transition>,
    }

    impl ObjectContent for ProgramContent {
        fn id(&self) -> Id { self.id }

        fn owner_id(&self) -> &Id { &self.owner_id }

        fn data(&self) -> Data {
            self.accepted_transitions
                .iter()
                .map(|(id, program)| transition_to_bytes(program).copy_from_slice(id.as_slice()))
                .flatten()
                .collect()
        }
    }

    impl ProgramContent {
        /// Creates a new Program object.
        pub fn new(
            version: u64,
            mutable: bool,
            owner_id: Id,
            accepted_transitions: Vec<Transition>,
        ) -> Self {
            let id = Self::generate_id(vec![
                version.to_be_bytes().to_vec(),
                vec![mutable as u8],
                owner_id.to_vec(),
                accepted_transitions
                    .iter()
                    .map(|it| transition_to_bytes(it))
                    .flatten()
                    .collect(),
            ]);

            let accepted_transitions = accepted_transitions
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
                accepted_transitions,
            }
        }
    }

    /// Converts a transition function into a byte vector.
    /// TODO - add code that converts the transition into bytes.
    fn transition_to_bytes(transition: &Transition) -> Vec<u8> {
        let mut result = vec![];

        result
    }

    /// Generates a unique ID for the transition function.
    /// Currently, we use SHA3-256 hash function to generate the ID.
    fn generate_transition_id(transition: &Transition) -> Id {
        let mut hasher = sha3::Sha3_256::new();
        hasher.update(transition_to_bytes(transition));
        let hash = hasher.finalize();

        Id(hash.into())
    }
}

pub(crate) mod data {
    use crate::space::object::{Data, ObjectContent};
    use crate::Id;

    /// A Data type of object.
    ///
    /// This object stores the data that can be changed as long as it is valid
    /// transition according to the constraining program.
    #[derive(Debug, Clone)]
    pub struct DataContent {
        /// Unique object ID
        id: Id,
        /// Flag if the program is mutable or not
        mutable: bool,
        /// Owner of the program. The owner describes how the program can
        /// evolve.
        owner_id: Id,
        /// Data object is storing
        pub data: Data,
    }

    impl ObjectContent for DataContent {
        fn id(&self) -> Id { self.id }

        fn owner_id(&self) -> &Id { &self.owner_id }

        fn data(&self) -> Data { self.data.clone() }
    }

    impl DataContent {
        /// Creates a new Data object.
        pub fn new(mutable: bool, owner_id: Id, data: Data) -> Self {
            let id = Self::generate_id(vec![vec![mutable as u8], owner_id.to_vec(), data.clone()]);
            Self {
                id,
                mutable,
                owner_id,
                data,
            }
        }

        pub fn transition(&self, data: Data) -> Self {
            let mut new = self.clone();
            new.data = data;
            new
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::space::object::data::DataContent;

    #[test]
    fn test_object_equality() {
        let owner_id = Id::default();

        let object = DataContent::new(false, owner_id, vec![1u8]);
        let object_changed = object.transition(vec![2u8]);

        assert_eq!(object.id(), object_changed.id());
        assert_eq!(Object::from(object), Object::from(object_changed));
    }

    #[test]
    fn test_object_id_generation() {
        let owner_id = Id::default();

        let object = DataContent::new(false, owner_id, vec![1u8]);

        let id = object.id();
        let expected_id =
            DataContent::generate_id(vec![vec![false as u8], owner_id.to_vec(), vec![1u8]]);

        assert_eq!(id, expected_id);
    }

    #[test]
    fn casting_downcasting() {
        let owner_id = Id::default();

        let wrapped_data_object = Object::Data(DataContent::new(false, owner_id, vec![1u8]));

        let wrapped_program_object =
            Object::Program(program::ProgramContent::new(0, false, owner_id, vec![]));

        for wrapped_object in vec![wrapped_data_object, wrapped_program_object] {
            let _res = match wrapped_object {
                Object::Data(_object) => (),
                Object::Program(_object) => (),
            };
        }
    }
}
