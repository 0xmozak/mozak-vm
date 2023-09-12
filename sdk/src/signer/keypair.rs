use std::error::Error;
use std::ops::Deref;

use bip39::{Language, Mnemonic, MnemonicType, Seed};
use ed25519_dalek::{SigningKey as Ed25519Keypair, SECRET_KEY_LENGTH};
use ed25519_dalek_bip32::{DerivationPath, ExtendedSigningKey};
use rand::rngs::OsRng;

#[derive(Clone, Debug)]
pub struct Keypair(Ed25519Keypair);

impl Keypair {
    /// Constructs a new, random `Keypair` using `OsRng`
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key: Ed25519Keypair = Ed25519Keypair::generate(&mut csprng);
        Keypair(signing_key)
    }

    /// Returns new mnemonic
    pub fn generate_mnemonic() -> Mnemonic {
        Mnemonic::new(MnemonicType::Words12, Language::English)
    }

    /// Returns new keypair from mnemonic and passphrase
    pub fn from_mnemonic(mnemonic: Mnemonic, passphrase: &str) -> Result<Keypair, Box<dyn Error>> {
        let seed = Seed::new(&mnemonic, passphrase);
        if seed.as_bytes().len() < SECRET_KEY_LENGTH {
            return Err("Seed is too short".into());
        }
        let mut bytes: [u8; SECRET_KEY_LENGTH] = [0u8; SECRET_KEY_LENGTH];
        bytes[..SECRET_KEY_LENGTH].copy_from_slice(&seed.as_bytes()[..SECRET_KEY_LENGTH]);

        let dalek_keypair = Ed25519Keypair::from_bytes(&bytes);
        Ok(Keypair(dalek_keypair))
    }

    /// Returns new keypair from derivation path, mnemonic and passphrase
    /// Derivation path is a string like "m/44'/60'/0'/0/1"
    pub fn from_mnemonic_derivation_path(
        mnemonic: Mnemonic,
        passphrase: &str,
        derivation_path: &DerivationPath,
    ) -> Result<Keypair, Box<dyn Error>> {
        let seed = Seed::new(&mnemonic, passphrase);
        let root = ExtendedSigningKey::from_seed(seed.as_bytes())?;
        let derived = root.derive(&derivation_path)?;
        Ok(Keypair(derived.signing_key))
    }
}

impl Deref for Keypair {
    type Target = Ed25519Keypair;

    fn deref(&self) -> &Self::Target { &self.0 }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_keypair_generate() {
        let keypair = Keypair::new();
        assert_eq!(keypair.to_bytes().len(), SECRET_KEY_LENGTH);
    }

    #[test]
    fn test_keypair_from_mnemonic() {
        let mnemonic = Keypair::generate_mnemonic();
        let keypair = Keypair::from_mnemonic(mnemonic, "").unwrap();
        assert_eq!(keypair.to_bytes().len(), SECRET_KEY_LENGTH);
    }

    #[test]
    fn test_keypair_from_mnemonic_derivation_path() {
        let mnemonic = Keypair::generate_mnemonic();
        let path = &DerivationPath::from_str("m/44'/60'/0'/0'").unwrap();
        let keypair = Keypair::from_mnemonic_derivation_path(mnemonic, "", path).unwrap();
        assert_eq!(keypair.to_bytes().len(), SECRET_KEY_LENGTH);
    }
}
