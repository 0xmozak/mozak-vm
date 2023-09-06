use std::{fmt, str};

/// PubKey for the Account, Program, and Objects
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PubKey(pub(crate) [u8; 32]);

impl fmt::Display for PubKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "PubKey: ({})",
            str::from_utf8(&self.0).expect("PubKey should be valid UTF-8 bytes")
        )
    }
}
