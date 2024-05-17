#![feature(restricted_std)]
#![allow(unused_attributes)]
extern crate alloc;

use mozak_sdk::common::types::{Poseidon2Hash, ProgramIdentifier, StateObject};
use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize, Serialize};

/// A generic private key used by the wallet.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct PrivateKey(pub [u8; 32]);

/// A generic public key. This is derived from the private key by
/// a poseidon2 hash.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct PublicKey(pub Poseidon2Hash);

impl From<[u8; 32]> for PrivateKey {
    fn from(value: [u8; 32]) -> Self { PrivateKey(value) }
}

impl PrivateKey {
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

impl From<StateObject> for TokenObject {
    fn from(value: StateObject) -> Self {
        let archived = unsafe { rkyv::access_unchecked::<TokenObject>(&value.data[..]) };
        let token_object: TokenObject = archived
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();
        token_object
    }
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

#[allow(clippy::unit_arg)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::ApproveSignature(pub_key, black_box) =>
            MethodReturns::ApproveSignature(approve_signature(pub_key, black_box)),
    }
}

#[allow(unused_variables)]
pub fn approve_signature<T>(pub_key: PublicKey, _black_box: T) {
    #[cfg(target_os = "mozakvm")]
    {
        let mut private_key_bytes = [0; 32];
        let _ = mozak_sdk::read(
            &mozak_sdk::InputTapeType::PrivateTape,
            &mut private_key_bytes[..],
        )
        .unwrap();
        let private_tape_pub_key = mozak_sdk::poseidon2_hash_no_pad(&private_key_bytes);
        assert!(private_tape_pub_key == pub_key.0);
    }

    #[cfg(not(target_os = "mozakvm"))]
    {}
}
