use crate::pubkey::PubKey;
use std::fmt::Display;
use std::fmt::Formatter;

pub struct Object {
    /// Unique object ID (Address)
    id: PubKey,
    /// Object version (each update increases the version)
    version: u64,
    /// Flag if the object is mutable or not
    mutable: bool,
    /// Owner of the object. Only the owner can modify the object.
    /// Owner can be account, another program, or the same as program field
    owner: PubKey,
    /// Program where Object belongs to and modified through
    program: PubKey,
    /// Data object is storing
    data: Vec<u8>,
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object ID: ( {} )", self.id)
    }
}
