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

/// A generic private key used by the wallet.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
pub struct PrivateKey([u8; 32]);

impl From<[u8; 32]> for PrivateKey {
    fn from(value: [u8; 32]) -> Self { PrivateKey(value) }
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

/// Hardcoded Pubkey
/// TODO(bing): delete
#[allow(dead_code)]
const PUB_KEY: PublicKey = PublicKey([
    21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181, 21, 0,
    31, 100, 7, 100, 189, 2, 100,
]);

const PRIV_KEY: PrivateKey = PrivateKey([
    21, 33, 31, 0, 7, 251, 189, 98, 22, 3, 1, 10, 71, 2, 90, 0, 1, 55, 55, 11, 62, 189, 181, 21, 0,
    31, 100, 7, 100, 189, 2, 100,
]);

// TODO(bing): Read private key from private tape.
// TODO(bing): Read public key from call tape.
// hash and compare against public key.
/// Return true if signature is approved.
pub fn approve_signature<T>(self_prog_id: ProgramIdentifier, pub_key: PublicKey, _black_box: T) -> () {
    // TODO(bing): Read private key from private tape
    let digest = poseidon2_hash(&PRIV_KEY.0);
    // assert_eq!(pub_key.0, *digest);

    // TODO(bing): Do we need to emit events here, even for the simplest
    // possible wallet that just approves signatures?
    //    event_emit(
    //        self_prog_id,
    //        mozak_sdk::coretypes::Event::ReadContextVariable(
    //
    // mozak_sdk::coretypes::ContextVariable::SelfProgramIdentifier(self_prog_id),
    //        ),
    //    );

    /// Only there for building cast list
       event_emit(
           self_prog_id,
           mozak_sdk::coretypes::Event::ReadStateObject(StateObject::default()),
       );
}
