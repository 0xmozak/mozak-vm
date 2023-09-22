use std::fmt;

use crate::pubkey::PubKey;

pub struct Object {
    /// Unique object ID (Address)
    id: PubKey,
    /// Object version (each update increases the version)
    version: u64,
    /// Flag if the object is mutable or not
    mutable: bool,
    /// Owner of the object. Only the owner program can modify the object.
    owner: PubKey,
    /// Data object is storing
    data: Vec<u8>,
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "Object ID: ({})", self.id) }
}
