use crate::message::VersionedMessage;
use crate::signature::Signature;

pub struct Transaction {
    /// List of signatures for the transaction
    pub signatures: Vec<Signature>,

    /// Message to sign
    pub message: VersionedMessage,
}
