#![feature(restricted_std)]
#![allow(unused_attributes)]
extern crate alloc;

use mozak_sdk::common::types::ProgramIdentifier;
use rkyv::{Archive, Deserialize, Serialize};

/// A generic public key used by the wallet.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct PublicKey([u8; 32]);

impl From<[u8; 32]> for PublicKey {
    fn from(value: [u8; 32]) -> Self { PublicKey(value) }
}

impl PublicKey {
    #[must_use]
    #[cfg(not(target_os = "mozakvm"))]
    /// To be removed later, when we have actual pubkeys
    pub fn new_from_rand_seed(seed: u64) -> Self {
        use rand::prelude::*;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut slice: [u8; 32] = [0; 32];
        rng.fill_bytes(&mut slice[..]);
        Self(slice)
    }
}

/// Amount of tokens to be used in a program, represented as part of
/// `TokenObject`.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct Amount(u64);

impl From<u64> for Amount {
    fn from(value: u64) -> Self { Amount(value) }
}

/// A token object is represented in the `data` section of a `StateObject`, and
/// contains information about the token that is being used in a program.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct TokenObject {
    /// The public key that is the economic owner of this `TokenObject`.
    pub pub_key: PublicKey,
    /// The amount of tokens to be used.
    pub amount: Amount,
}

/// A generic 'black box' object that can contain any
/// data that a guest program writer wants.
///
/// The purpose of this 'black box' is to ensure the uniqueness of the
/// merkle caps generated, which allows us (in this particular use case) to
/// differentiate between transactions.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct BlackBox {
    pub remitter_program: ProgramIdentifier,
    pub remittee_program: ProgramIdentifier,
    pub token_object: TokenObject,
}

impl BlackBox {
    pub fn new(
        remitter_program: ProgramIdentifier,
        remittee_program: ProgramIdentifier,
        token_object: TokenObject,
    ) -> Self {
        BlackBox {
            remitter_program,
            remittee_program,
            token_object,
        }
    }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    ApproveSignature(PublicKey, BlackBox),
}

#[derive(Archive, Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum MethodReturns {
    ApproveSignature(()),
}

// TODO: Remove later
impl Default for MethodReturns {
    fn default() -> Self { Self::ApproveSignature(()) }
}

pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::ApproveSignature(pub_key, black_box) =>
            MethodReturns::ApproveSignature(approve_signature(pub_key, black_box)),
    }
}

// TODO(bing): Read private key from private tape and public key from call tape.
// hash and compare against public key.
pub fn approve_signature<T>(_pub_key: PublicKey, _black_box: T) -> () {
    // Null for now
}
