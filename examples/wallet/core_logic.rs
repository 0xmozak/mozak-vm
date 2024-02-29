extern crate alloc;
use mozak_sdk::coretypes::ProgramIdentifier;
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

/// Amount of tokens to be used in a program, represented as part of
/// `TokenObject`.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct Amount(u64);

impl From<u64> for Amount {
    fn from(value: u64) -> Self { Amount(value) }
}

/// A token object is represented in the `data` section of a `StateObject`, and
/// contains information about the token that is being used in a program.
#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct TokenObject {
    /// The wallet that is the economic owner of this `TokenObject`.
    pub wallet_prog_id: ProgramIdentifier,
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
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct BlackBox {
    pub remitter_wallet: ProgramIdentifier,
    pub remittee_wallet: ProgramIdentifier,
    pub token_object: TokenObject,
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

// TODO(bing): Read private key from private tape and public key from call tape.
// hash and compare against public key.
/// Return true if signature is approved.
pub fn approve_signature<T>(
    _self_prog_id: ProgramIdentifier,
    _pub_key: PublicKey,
    _black_box: T,
) -> () {
    // Null for now
}
