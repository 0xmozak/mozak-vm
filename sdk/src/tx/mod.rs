use crate::signature::Signature;
use crate::message::VersionedMessage;

pub struct Transaction {
    /// List of signatures for the transaction
    pub signatures: Vec<Signature>,

    /// Message to sign
    pub message: VersionedMessage,
}
