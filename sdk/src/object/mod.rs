use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use sha3::Digest;

pub mod data;
pub mod program;

use super::Id;

/// Generic data representation, that all objects should be able to convert to.
type Data = Vec<u8>;

/// A generic object type.
/// It can be either a Program or Data object.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Object {
    Program(program::ProgramContent),
    Data(data::DataContent),
}

#[cfg(not(feature = "no-std"))]
impl Default for Object {
    fn default() -> Self { Object::Program(program::ProgramContent::default()) }
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
    pub fn id(&self) -> Id {
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

/// Our application network is a collection of objects of different types, which
/// depends on the type of data that is stored in the object.
pub trait ObjectContent: Clone {
    /// The unique id of the object, can be considered as an object address.
    ///
    /// It is generated deterministically based on the constraining program id,
    /// and some additional parameters that are provided during the object
    /// creation.
    fn id(&self) -> Id;

    /// Id of an object that controls how the object can evolve.
    /// Typically, this object would be a program that has a list of state
    /// transitions that are allowed.
    fn owner(&self) -> &Id;

    /// Generates a unique ID for the object.
    /// Currently, we use SHA3-256 hash function to generate the ID.
    #[cfg(feature = "std")]
    fn generate_id<T: Into<Box<[u8]>> + Clone>(parameters: Vec<T>) -> Id {
        let mut hasher = sha3::Sha3_256::new();
        parameters
            .iter()
            .for_each(|p| hasher.update(p.clone().into()));
        let hash = hasher.finalize();

        Id(hash.into())
    }
}

#[cfg(all(test, feature = "std"))]
mod test {
    use super::*;
    use crate::object::data::DataContent;

    #[test]
    fn test_object_equality() {
        let owner = Id::default();

        let object = DataContent::new(false, owner, vec![1u8]);
        let object_changed = object.transition(vec![2u8]);

        assert_eq!(object.id(), object_changed.id());
        assert_eq!(Object::from(object), Object::from(object_changed));
    }

    #[test]
    fn test_object_id_generation() {
        let owner = Id::default();

        let object = DataContent::new(false, owner, vec![1u8]);

        let id = object.id();
        let expected_id =
            DataContent::generate_id(vec![vec![false as u8], owner.to_vec(), vec![1u8]]);

        assert_eq!(id, expected_id);
    }

    #[test]
    fn casting_downcasting() {
        let owner = Id::default();

        let wrapped_data_object = Object::Data(DataContent::new(false, owner, vec![1u8]));

        let wrapped_program_object =
            Object::Program(program::ProgramContent::new(0, false, owner, vec![]));

        for wrapped_object in vec![wrapped_data_object, wrapped_program_object] {
            match wrapped_object {
                Object::Data(_object) => (),
                Object::Program(_object) => (),
            };
        }
    }
}
