use crate::pubkey::PubKey;

pub struct Program {
    /// Unique program ID (Address)
	id: PubKey,
    /// Program version (each update increases the version)
	version: u64,
	/// Flag if the program is mutable or not
	mutable: bool,
	/// Owner of the program. Only the owner can modify the program.
	/// Owner can be account, another program, or the same as program field
	owner: PubKey,
	/// Program code
	data: Vec<u8>,
}
