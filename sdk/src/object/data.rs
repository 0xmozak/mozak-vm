use serde::{Deserialize, Serialize};

extern crate alloc;

use crate::object::{Data, ObjectContent};
use crate::Id;

/// A Data type of object.
///
/// This object stores the data that can be changed as long as it is valid
/// transition according to the constraining program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataContent {
    /// Unique object ID
    id: Id,
    /// Flag if the object is mutable or not
    mutable: bool,
    /// Owner of the object. The owner describes how the object can change.
    owner: Id,
    /// Raw data for the object
    pub data: Data,
}

impl ObjectContent for DataContent {
    fn id(&self) -> Id { self.id }

    fn owner(&self) -> &Id { &self.owner }
}

impl DataContent {
    /// Creates a new Data object.
    #[cfg(feature = "std")]
    pub fn new(mutable: bool, owner: Id, data: Data) -> Self {
        let id = Self::generate_id(vec![vec![mutable as u8], owner.to_vec(), data.clone()]);
        Self {
            id,
            mutable,
            owner,
            data,
        }
    }

    pub fn transition(&self, data: Data) -> Self {
        let mut new = self.clone();
        new.data = data;
        new
    }
}
