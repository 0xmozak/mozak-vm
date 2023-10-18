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
}

impl Default for Keypair {
    /// Constructs a new, random `Keypair` using `OsRng`
    fn default() -> Self { Keypair(Ed25519Keypair::generate(&mut OsRng)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate() {
        let keypair = Keypair::default();
        assert_eq!(keypair.0.to_bytes().len(), Keypair::SECRET_KEY_LENGTH);
    }
}
