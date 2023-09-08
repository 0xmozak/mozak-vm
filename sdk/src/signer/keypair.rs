use std::ops::Deref;

use ed25519_dalek::SigningKey as Ed25519Keypair;
use rand::rngs::OsRng;

#[derive(Clone, Debug)]
pub struct Keypair(Ed25519Keypair);

impl Keypair {
    /// The length of an ed25519 `Keypair`, in bytes.
    pub const KEYPAIR_LENGTH: usize = Self::SECRET_KEY_LENGTH + Self::PUBLIC_KEY_LENGTH;
    /// The length of an ed25519 `PublicKey`, in bytes.
    pub const PUBLIC_KEY_LENGTH: usize = 32;
    /// The length of a ed25519 `SecretKey`, in bytes.
    pub const SECRET_KEY_LENGTH: usize = 32;
    /// The length of a ed25519 `Signature`, in bytes.
    pub const SIGNATURE_LENGTH: usize = 64;

    /// Constructs a new, random `Keypair` using `OsRng`
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key: Ed25519Keypair = Ed25519Keypair::generate(&mut csprng);
        Keypair(signing_key)
    }
}

impl Deref for Keypair {
    type Target = Ed25519Keypair;

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate() {
        let keypair = Keypair::new();
        assert_eq!(keypair.to_bytes().len(), Keypair::SECRET_KEY_LENGTH);
    }
}
