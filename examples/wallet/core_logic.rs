#![feature(restricted_std)]
extern crate alloc;
use guest::hash::poseidon2_hash;
// use alloc::vec::Vec;
use mozak_sdk::coretypes::{ProgramIdentifier, StateObject};
use mozak_sdk::sys::event_emit;
use rkyv::{Archive, Deserialize, Serialize};

/// A generic public key used by the wallet.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct PublicKey([u8; 32]);

impl From<[u8; 32]> for PublicKey {
    fn from(value: [u8; 32]) -> Self { PublicKey(value) }
}

/// Amount of tokens to transfer.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct Amount(u64);

impl From<u64> for Amount {
    fn from(value: u64) -> Self { Amount(value) }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct TokenObject {
    pub wallet_prog_id: ProgramIdentifier,
    pub pub_key: PublicKey,
    pub amount: Amount,
}

/// A generic 'black box' object that can contain any
/// data that a guest program writer wants.
///
/// The purpose of this 'black box' is to ensure the uniqueness of the
/// merkle caps generated, which allows us (in this particular use case) to
/// differentiate between transactions.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct BlackBox {
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
    token_object: TokenObject,
}

impl BlackBox {
    pub fn new(
        remitter_wallet: ProgramIdentifier,
        remittee_wallet: ProgramIdentifier,
        token_object: TokenObject,
    ) -> Self {
        BlackBox {
            remitter_wallet,
            remittee_wallet,
            token_object,
        }
    }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    ApproveSignature(ProgramIdentifier, PublicKey, BlackBox),
}

#[derive(Archive, Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub enum MethodReturns {
    ApproveSignature(()),
}

// TODO: Remove later
impl Default for MethodReturns {
    fn default() -> Self { Self::ApproveSignature(()) }
}

pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::ApproveSignature(id, pub_key, black_box) =>
            MethodReturns::ApproveSignature(approve_signature(id, pub_key, black_box)),
    }
}

// TODO(bing): Read private key from private tape.
// TODO(bing): Read public key from call tape.
// hash and compare against public key.
/// Return true if signature is approved.
pub fn approve_signature<T>(
    self_prog_id: ProgramIdentifier,
    pub_key: PublicKey,
    _black_box: T,
) -> () {
    // Null for now
}
