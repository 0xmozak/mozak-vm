//! Circuits for proving correspondence of all parts of a block

use std::marker::PhantomData;

use anyhow::Result;
use itertools::Either;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierOnlyCircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{match_delta, state_update, verify_tx};

pub mod core;

#[derive(Clone)]
pub struct Indices {
    pub block: core::PublicIndices,
}

pub trait IsBase {
    const VALUE: bool;
}

#[derive(Copy, Clone, Debug)]
pub struct Base;

impl IsBase for Base {
    const VALUE: bool = true;
}

#[derive(Copy, Clone, Debug)]
pub struct Block;

impl IsBase for Block {
    const VALUE: bool = false;
}

pub type Proof<T, F, C, const D: usize> = super::Proof<T, Indices, F, C, D>;

pub type BaseProof<F, C, const D: usize> = Proof<Base, F, C, D>;

pub type BlockProof<F, C, const D: usize> = Proof<Block, F, C, D>;

pub enum BaseOrBlockRef<'a, F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    Base(&'a Proof<Base, F, C, D>),
    Block(&'a Proof<Block, F, C, D>),
}

impl<'a, F, C, const D: usize> Clone for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn clone(&self) -> Self { *self }
}

impl<'a, F, C, const D: usize> Copy for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
}

impl<'a, F, C, const D: usize> From<&'a Proof<Base, F, C, D>> for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a Proof<Base, F, C, D>) -> Self { Self::Base(value) }
}

impl<'a, F, C, const D: usize> From<&'a mut Proof<Base, F, C, D>> for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a mut Proof<Base, F, C, D>) -> Self { Self::Base(value) }
}

impl<'a, F, C, const D: usize> From<&'a Proof<Block, F, C, D>> for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a Proof<Block, F, C, D>) -> Self { Self::Block(value) }
}

impl<'a, F, C, const D: usize> From<&'a mut Proof<Block, F, C, D>> for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a mut Proof<Block, F, C, D>) -> Self { Self::Block(value) }
}

impl<'a, F, C, const D: usize> From<&'a Either<Proof<Base, F, C, D>, Proof<Block, F, C, D>>>
    for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a Either<Proof<Base, F, C, D>, Proof<Block, F, C, D>>) -> Self {
        match value {
            Either::Left(l) => Self::Base(l),
            Either::Right(b) => Self::Block(b),
        }
    }
}

impl<'a, F, C, const D: usize> From<&'a mut Either<Proof<Base, F, C, D>, Proof<Block, F, C, D>>>
    for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a mut Either<Proof<Base, F, C, D>, Proof<Block, F, C, D>>) -> Self {
        match value {
            Either::Left(l) => Self::Base(l),
            Either::Right(b) => Self::Block(b),
        }
    }
}

impl<'a, F, C, const D: usize> From<Either<&'a Proof<Base, F, C, D>, &'a Proof<Block, F, C, D>>>
    for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: Either<&'a Proof<Base, F, C, D>, &'a Proof<Block, F, C, D>>) -> Self {
        match value {
            Either::Left(l) => Self::Base(l),
            Either::Right(b) => Self::Block(b),
        }
    }
}

impl<'a, F, C, const D: usize> From<&'a Either<Proof<Block, F, C, D>, Proof<Base, F, C, D>>>
    for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a Either<Proof<Block, F, C, D>, Proof<Base, F, C, D>>) -> Self {
        match value {
            Either::Left(b) => Self::Block(b),
            Either::Right(l) => Self::Base(l),
        }
    }
}

impl<'a, F, C, const D: usize> From<&'a mut Either<Proof<Block, F, C, D>, Proof<Base, F, C, D>>>
    for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: &'a mut Either<Proof<Block, F, C, D>, Proof<Base, F, C, D>>) -> Self {
        match value {
            Either::Left(b) => Self::Block(b),
            Either::Right(l) => Self::Base(l),
        }
    }
}

impl<'a, F, C, const D: usize> From<Either<&'a Proof<Block, F, C, D>, &'a Proof<Base, F, C, D>>>
    for BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(value: Either<&'a Proof<Block, F, C, D>, &'a Proof<Base, F, C, D>>) -> Self {
        match value {
            Either::Left(b) => Self::Block(b),
            Either::Right(l) => Self::Base(l),
        }
    }
}

impl<'a, F, C, const D: usize> BaseOrBlockRef<'a, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    pub const fn is_base(&self) -> bool {
        match self {
            Self::Base(_) => Base::VALUE,
            Self::Block(_) => Block::VALUE,
        }
    }

    pub const fn proof(&self) -> &ProofWithPublicInputs<F, C, D> {
        match self {
            Self::Base(l) => &l.proof,
            Self::Block(b) => &b.proof,
        }
    }

    pub const fn indices(&self) -> &Indices {
        match self {
            Self::Base(l) => &l.indices,
            Self::Block(b) => &b.indices,
        }
    }
}

impl<T, F, C, const D: usize> Proof<T, F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    C::Hasher: Hasher<F, Hash = HashOut<F>>,
{
    pub fn verifier(&self) -> VerifierOnlyCircuitData<C, D> {
        self.indices
            .block
            .verifier
            .get_field(&self.proof.public_inputs)
    }

    pub fn base_state(&self) -> HashOut<F> {
        self.indices
            .block
            .base_state_root
            .get_field(&self.proof.public_inputs)
    }

    pub fn state(&self) -> HashOut<F> {
        self.indices
            .block
            .state_root
            .get_field(&self.proof.public_inputs)
    }

    pub fn block_height(&self) -> u64 {
        self.indices
            .block
            .block_height
            .get_field(&self.proof.public_inputs)
            .to_canonical_u64()
    }
}

pub struct Circuit<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    /// The tx verifier
    pub tx: core::TxVerifierSubCircuit<D>,

    /// The match delta verifier
    pub match_delta: core::MatchDeltaVerifierSubCircuit<D>,

    /// The state update verifier
    pub state_update: core::StateUpdateVerifierSubCircuit<D>,

    /// The block verifier
    pub block: core::SubCircuit<F, C, D>,

    pub circuit: CircuitData<F, C, D>,
}

impl<F, C, const D: usize> Circuit<F, C, D>
where
    F: RichField + Extendable<D>,
    C: 'static + GenericConfig<D, F = F>,
    <C as GenericConfig<D>>::Hasher: AlgebraicHasher<F>,
{
    #[must_use]
    pub fn new(
        circuit_config: &CircuitConfig,
        tx: &verify_tx::BranchCircuit<F, C, D>,
        md: &match_delta::BranchCircuit<F, C, D>,
        su: &state_update::BranchCircuit<F, C, D>,
    ) -> Self {
        let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

        let block_inputs = core::SubCircuitInputs::default(&mut builder);

        let tx_targets = core::TxVerifierTargets::build_targets(&mut builder, tx);
        let match_delta_targets = core::MatchDeltaVerifierTargets::build_targets(&mut builder, md);
        let state_update_targets =
            core::StateUpdateVerifierTargets::build_targets(&mut builder, su);
        let block = block_inputs.build(&mut builder);

        builder.connect_hashes(tx_targets.event_root, match_delta_targets.event_root);
        builder.connect(match_delta_targets.block_height, block.inputs.block_height);
        builder.connect_hashes(
            match_delta_targets.state_delta,
            state_update_targets.summary_root,
        );
        builder.connect_hashes(state_update_targets.old_root, block.prev_state_root);
        builder.connect_hashes(state_update_targets.new_root, block.inputs.state_root);

        let circuit = builder.build();

        let public_inputs = &circuit.prover_only.public_inputs;
        let tx = tx_targets.build(public_inputs);
        let match_delta = match_delta_targets.build(public_inputs);
        let state_update = state_update_targets.build(public_inputs);

        Self {
            tx,
            match_delta,
            state_update,
            block,
            circuit,
        }
    }

    fn indices(&self) -> Indices {
        Indices {
            block: self.block.indices.clone(),
        }
    }

    pub fn prove_base(&self, base_state_root: HashOut<F>) -> Result<BaseProof<F, C, D>> {
        let proof = self
            .block
            .prove_base(&self.circuit.verifier_only, base_state_root)?;
        Ok(BaseProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn verify_base(&self, base_proof: BaseProof<F, C, D>) -> Result<()> {
        self.block.verify_base(base_proof.proof)
    }

    pub fn prove<'a>(
        &self,
        tx_proof: &verify_tx::BranchProof<F, C, D>,
        match_proof: &match_delta::BranchProof<F, C, D>,
        state_proof: &state_update::BranchProof<F, C, D>,
        prev_proof: impl Into<BaseOrBlockRef<'a, F, C, D>>,
    ) -> Result<BlockProof<F, C, D>> {
        let mut inputs = PartialWitness::new();
        self.tx.set_witness(&mut inputs, tx_proof);
        self.match_delta.set_witness(&mut inputs, match_proof);
        self.state_update.set_witness(&mut inputs, state_proof);
        inputs.set_proof_with_pis_target(&self.block.prev_proof, prev_proof.into().proof());
        let proof = self.circuit.prove(inputs)?;
        Ok(BlockProof {
            proof,
            tag: PhantomData,
            indices: self.indices(),
        })
    }

    pub fn verify(&self, proof: BlockProof<F, C, D>) -> Result<()> {
        self.circuit.verify(proof.proof)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::circuits::match_delta::test as match_delta;
    use crate::circuits::state_update::test as state_update;
    use crate::circuits::test_data::{STATE_0_ROOT_HASH, STATE_1_ROOT_HASH};
    use crate::circuits::verify_tx::test as verify_tx;
    use crate::test_utils::{C, CONFIG, D, F, NON_ZERO_HASHES, ZERO_HASH};

    #[tested_fixture::tested_fixture(CIRCUIT)]
    fn build_circuit() -> Circuit<F, C, D> {
        Circuit::new(
            &CONFIG,
            *verify_tx::BRANCH,
            *match_delta::BRANCH,
            *state_update::BRANCH_3,
        )
    }

    fn assert_proof<T>(
        proof: &Proof<T, F, C, D>,
        base_root: HashOut<F>,
        root: HashOut<F>,
        block_height: u64,
    ) {
        let p_base_root = proof.base_state();
        assert_eq!(p_base_root, base_root);

        let p_root = proof.state();
        assert_eq!(p_root, root);

        let p_block_height = proof.block_height();
        assert_eq!(p_block_height, block_height);
    }

    #[test]
    fn verify_zero_base() -> Result<()> {
        let proof = CIRCUIT.prove_base(ZERO_HASH)?;
        assert_proof(&proof, ZERO_HASH, ZERO_HASH, 0);
        CIRCUIT.verify_base(proof.clone())?;
        Ok(())
    }

    #[test]
    fn verify_non_zero_base() -> Result<()> {
        let proof = CIRCUIT.prove_base(NON_ZERO_HASHES[0])?;
        assert_proof(&proof, NON_ZERO_HASHES[0], NON_ZERO_HASHES[0], 0);
        CIRCUIT.verify_base(proof.clone())?;
        Ok(())
    }

    #[tested_fixture::tested_fixture(STATE_0_BASE_PROOF: BaseProof<F, C, D>)]
    fn verify_state_0_base() -> Result<BaseProof<F, C, D>> {
        let proof = CIRCUIT.prove_base(*STATE_0_ROOT_HASH)?;
        assert_proof(&proof, *STATE_0_ROOT_HASH, *STATE_0_ROOT_HASH, 0);
        CIRCUIT.verify_base(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify() -> Result<()> {
        let proof = CIRCUIT.prove(
            *verify_tx::BRANCH_PROOF,
            *match_delta::BRANCH_PROOF,
            *state_update::ROOT_PROOF,
            *STATE_0_BASE_PROOF,
        )?;
        assert_proof(&proof, *STATE_0_ROOT_HASH, *STATE_1_ROOT_HASH, 1);
        CIRCUIT.verify(proof)?;
        Ok(())
    }
}
