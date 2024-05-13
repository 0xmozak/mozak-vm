use itertools::{chain, Itertools};
use mozak_circuits::memoryinit::generation::generate_elf_memory_init_trace;
use mozak_circuits::program::generation::generate_program_rom_trace;
use mozak_circuits::stark::utils::trace_rows_to_poly_values;
use mozak_runner::elf::Program;
use mozak_sdk::common::types::{Poseidon2Hash, ProgramIdentifier};
use plonky2::field::extension::Extendable;
use plonky2::field::types::Field;
use plonky2::fri::oracle::PolynomialBatch;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::{
    AlgebraicHasher, GenericConfig, GenericHashOut, Hasher, Poseidon2GoldilocksConfig,
};
use plonky2::util::timing::TimingTree;
use starky::config::StarkConfig;

pub struct ProgIdBytes(Poseidon2Hash);

// TODO: have a separate type for these in circuits
pub const PRODUCTION_STARK_CONFIG: StarkConfig = StarkConfig::standard_fast_config();
pub type ProductionGenericConfig = Poseidon2GoldilocksConfig;
pub type F = <ProductionGenericConfig as GenericConfig<D>>::F;
pub const D: usize = 2;
// TODO: remove this once we have a way to access ELF path inside native
pub const ELF_DIR: &str = "../target/riscv32im-mozak-mozakvm-elf/release/"; // relative to example in /examples

impl ProgIdBytes {
    pub fn inner(&self) -> Poseidon2Hash { self.0 }

    /// Create [ProgIdBytes] from `program` using production configs for stark
    /// and recursive plonky2 prover
    pub fn from_production_configs(program: &Program) -> Self {
        let entry_point = F::from_canonical_u32(program.entry_point);
        let elf_memory_init_trace = generate_elf_memory_init_trace::<F>(&program);
        let program_rom_trace = generate_program_rom_trace::<F>(&program);
        let elf_memory_init_hash = get_trace_commitment_hash::<F, ProductionGenericConfig, D, _>(
            elf_memory_init_trace,
            &PRODUCTION_STARK_CONFIG,
        );
        let program_hash = get_trace_commitment_hash::<F, Poseidon2GoldilocksConfig, D, _>(
            program_rom_trace,
            &PRODUCTION_STARK_CONFIG,
        );
        let hashout =
            <<ProductionGenericConfig as GenericConfig<D>>::InnerHasher as Hasher<F>>::hash_pad(
                &chain!(
                    [entry_point],
                    program_hash.elements,
                    elf_memory_init_hash.elements
                )
                .collect_vec(),
            );
        Self(hashout.to_bytes().try_into().unwrap())
    }

    pub fn from_elf(elf_path: &str) -> anyhow::Result<Self> {
        let elf_bytes = std::fs::read(elf_path)?;
        let program = Program::mozak_load_program(&elf_bytes)?;

        Ok(Self::from_production_configs(&program))
    }
}

impl Into<ProgramIdentifier> for ProgIdBytes {
    fn into(self) -> ProgramIdentifier { ProgramIdentifier(self.inner().into()) }
}
/// Compute merkle cap of the trace, and return its hash.
fn get_trace_commitment_hash<F, C, const D: usize, Row: IntoIterator<Item = F>>(
    trace: Vec<Row>,
    config: &StarkConfig,
) -> <<C as GenericConfig<D>>::InnerHasher as Hasher<F>>::Hash
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: AlgebraicHasher<F>, {
    let trace_poly_values = trace_rows_to_poly_values(trace);
    let rate_bits = config.fri_config.rate_bits;
    let cap_height = config.fri_config.cap_height;
    let trace_commitment = PolynomialBatch::<F, C, D>::from_values(
        trace_poly_values,
        rate_bits,
        false,
        cap_height,
        &mut TimingTree::default(),
        None,
    );
    let merkle_cap = trace_commitment.merkle_tree.cap;
    <<C as GenericConfig<D>>::InnerHasher as Hasher<F>>::hash_pad(&merkle_cap.flatten())
}
