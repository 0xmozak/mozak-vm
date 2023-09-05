use crate::pubkey::PubKey;
use crate::instruction::CompressedInstruction;

pub struct Message {
    /// All the account keys used by this transaction
    pub accounts: Vec<PubKey>,

    /// The list of program instructions that will be executed
    pub instructions: Vec<CompressedInstruction>,
}
