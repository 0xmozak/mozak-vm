use std::fmt::Display;
use std::fmt::Formatter;

/// PubKey for the Account, Program, and Objects
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct PubKey(pub(crate) [u8; 32]);

impl Display for PubKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let bytes = self.0.to_vec();
        write!(f, "PubKey: ( {} )", String::from_utf8(bytes).unwrap())
    }
}
