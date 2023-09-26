use serde::{Deserialize, Serialize};

use crate::object::ObjectContent;
use crate::Id;

/// Generic data representation, that all objects should be able to convert to.
type Data = Vec<u8>;

/// A Data type of object.
///
/// This object stores the data that can be changed as long as it is valid
/// transition according to the constraining program.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
