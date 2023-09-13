/// We need to bridge the idea of a program that can access multiple blobs and a
/// RISC-V program, that just has a memory region it can read and write to.
///
/// We do so by first listing the blobs that the program reads/writes to. This
/// requires running the program in a simulation mode to identify the blobs that
/// are read/written to.
///
/// We then combine the read blobs together into a single `read-from` table,
/// and combine the write blobs together into a single `write-to` table.
/// We then prove that the program indeed, when run on the `read-from`,
/// `write-to` tables terminates with the `write-to` table state. For that we
/// use the `vm-prover`.
///
/// Fiin
pub struct ProgramRunProof {
    proof: StarkProof,
    public_input: StarkPublicInput,
}

struct StarkPublicInput {
    inputs: Vec<u8>,
    initial_state: Vec<u8>,
    final_state: Vec<u8>,
}

struct StarkProof;
