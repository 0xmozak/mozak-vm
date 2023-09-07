use rand::rngs::OsRng;
use ed25519_dalek::SigningKey as Ed25519Keypair;

#[derive(Clone, Debug)]
pub struct Keypair(Ed25519Keypair);

impl Keypair {
    /// The length of a ed25519 `Signature`, in bytes.
    pub const SIGNATURE_LENGTH: usize = 64;

    /// The length of a ed25519 `SecretKey`, in bytes.
    pub const SECRET_KEY_LENGTH: usize = 32;

    /// The length of an ed25519 `PublicKey`, in bytes.
    pub const PUBLIC_KEY_LENGTH: usize = 32;

    /// The length of an ed25519 `Keypair`, in bytes.
    pub const KEYPAIR_LENGTH: usize = Self::SECRET_KEY_LENGTH + Self::PUBLIC_KEY_LENGTH;

    /// Constructs a new, random `Keypair` using `OsRng`
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key: Ed25519Keypair = Ed25519Keypair::generate(csprng);
        Keypair(signing_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate() {
        let mut rng = OsRng;
        let keypair = Keypair::generate(&mut rng);
        assert_eq!(keypair.0.secret.to_bytes().len(), Keypair::SECRET_KEY_LENGTH);
    }
}
