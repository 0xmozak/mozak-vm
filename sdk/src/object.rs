use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::Data;

/// A Data Object.
///
/// This object stores the data that can be changed by the owner program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Unique object ID
    pub id: Id,
    /// Flag if the object is mutable or not
    pub mutable: bool,
    /// Owner of the object. The owner describes how the object can change.
    pub owner: Id,
    /// Raw data for the object
    pub data: Data,
}

impl Object {
    /// Creates a new Data object.
    #[must_use]
    pub fn new(mutable: bool, owner: Id, data: Data, seed: u64) -> Self {
        let id = Id::derive(owner, seed);
        Self {
            id,
            mutable,
            owner,
            data,
        }
    }

    /// Updates the obj and return new obj
    #[must_use]
    pub fn update(&self, data: Data) -> Self {
        let mut obj = self.clone();
        obj.data = data;
        obj
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "Object ID: ({})", self.id) }
}

mod test {
    #[allow(clippy::bool_assert_comparison)]
    #[test]
    fn test_object() {
        use super::*;

        let owner = Id::random();
        let data = vec![1, 2, 3];
        let obj = Object::new(true, owner, data, 0);
        assert_eq!(obj.data, vec![1, 2, 3]);
        assert_eq!(obj.mutable, true);
        assert_eq!(obj.owner, owner);
    }
}
