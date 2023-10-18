#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
pub mod account;
pub mod crypto;
pub mod id;
pub mod instruction;
pub mod message;
pub mod object;
pub mod program;
pub mod pubkey;
pub mod signature;
pub mod signer;
pub mod tx;

/// Generic data representation, that all objects should be able to convert to.
type Data = Vec<u8>;
