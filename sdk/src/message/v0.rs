use crate::instruction::CompressedInstruction;
use crate::pubkey::PubKey;

use super::message_header::MessageHeader;

pub struct Message {
    /// The message header, identifying signed and read-only `account_keys`
    /// indices and indicating how many program `instructions` will follow.
    pub header: MessageHeader,

    /// All the account keys used by this transaction
    pub accounts: Vec<PubKey>,

    /// The list of program instructions that will be executed
    pub instructions: Vec<CompressedInstruction>,
}
