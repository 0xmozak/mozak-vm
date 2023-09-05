use crate::pubkey::PubKey;

pub struct Instruction {
    /// Pubkey of the program that executes this instruction
    pub program_id: PubKey,
    /// Metadata describing accounts that should be passed to the program.
    pub accounts: Vec<AccountMeta>,
    /// Opaque data passed to the program for its own interpretation.
    pub data: Vec<u8>,
}

pub struct CompressedInstruction {
    /// Index into the transaction keys array indicating the program account that executes this instruction.
    pub program_id_index: u8,
    /// Ordered indices into the transaction keys array indicating which accounts to pass to the program.
    pub accounts: Vec<u8>,
    /// The program input data.
    pub data: Vec<u8>,
}

pub struct AccountMeta {
    /// An account's public key.
    pub pubkey: PubKey,
    /// True if an `Instruction` requires a `Transaction` signature matching `pubkey`.
    pub is_signer: bool,
    /// True if the account data or metadata may be mutated during program execution.
    pub is_writable: bool,
}