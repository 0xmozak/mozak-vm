use anyhow::{bail, Result};
use itertools::Either;
use mozak_recproofs::circuits::{match_delta, state_update, verify_block, verify_tx};
use plonky2::hash::hash_types::HashOut;
use plonky2::plonk::circuit_data::CircuitConfig;

use super::matches::AuxMatchesData;
use super::state::AuxStateData;
use super::transactions::AuxTransactionData;
use crate::{C, D, F};

type TxProof = verify_tx::BranchProof<F, C, D>;
type StateProof = state_update::BranchProof<F, C, D>;
type MatchProof = match_delta::BranchProof<F, C, D>;

type BlockCircuit = verify_block::Circuit<F, C, D>;
type BaseProof = verify_block::BaseProof<F, C, D>;
type BlockProof = verify_block::BlockProof<F, C, D>;

pub struct AuxBlockData {
    circuit: BlockCircuit,
}

impl AuxBlockData {
    /// Create the auxiliary block data. This includes all the circuits
    /// and dummy proofs. This only needs to be done once, as multiple
    /// `Blocks`s can use the same `AuxBlockData`.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(
        config: &CircuitConfig,
        tx: &AuxTransactionData,
        md: &AuxMatchesData,
        su: &AuxStateData,
    ) -> Self {
        let circuit = BlockCircuit::new(
            config,
            &tx.tx_branch_circuit,
            &md.match_branch_circuit,
            su.branch_circuits.last().unwrap(),
        );

        Self { circuit }
    }
}

pub struct Blocks<'a> {
    aux: &'a AuxBlockData,
    proof: Either<BaseProof, BlockProof>,
}

impl<'a> Blocks<'a> {
    /// Create the first block
    ///
    /// # Panics
    ///
    /// Panics if the circuit logic has a bug.
    #[must_use]
    pub fn new(aux: &'a AuxBlockData, state: HashOut<F>) -> Self {
        let proof = Either::Left(aux.circuit.prove_base(state).unwrap());
        Self { aux, proof }
    }

    /// Get the current block height
    #[must_use]
    pub fn height(&self) -> u64 {
        match &self.proof {
            Either::Left(p) => p.block_height(),
            Either::Right(p) => p.block_height(),
        }
    }

    /// Increment to the next block.
    ///
    /// # Errors
    ///
    /// Returns an error if the proofs don't match.
    pub fn increment(
        &mut self,
        tx_proof: &TxProof,
        match_proof: &MatchProof,
        state_proof: &StateProof,
    ) -> Result<()> {
        let Ok(proof) = self
            .aux
            .circuit
            .prove(tx_proof, match_proof, state_proof, &self.proof)
        else {
            bail!("Proofs mismatched")
        };
        self.proof = Either::Right(proof);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{AuxBlockData, Blocks};
    use crate::block_proposer::matches::test as matches;
    use crate::block_proposer::state::test as state;
    use crate::block_proposer::test_data::CONFIG;
    use crate::block_proposer::transactions::test as transactions;

    #[tested_fixture::tested_fixture(AUX_0)]
    fn build_aux_0() -> AuxBlockData {
        AuxBlockData::new(&CONFIG, *transactions::AUX, *matches::AUX, *state::AUX_0)
    }

    #[tested_fixture::tested_fixture(AUX_8)]
    fn build_aux_8() -> AuxBlockData {
        AuxBlockData::new(&CONFIG, *transactions::AUX, *matches::AUX, *state::AUX_8)
    }

    #[tested_fixture::tested_fixture(AUX_63)]
    fn build_aux_63() -> AuxBlockData {
        AuxBlockData::new(&CONFIG, *transactions::AUX, *matches::AUX, *state::AUX_63)
    }

    #[test]
    fn simple_0() {
        let mut blocks = Blocks::new(*AUX_0, state::SIMPLE_0.old());

        blocks
            .increment(*transactions::SIMPLE, *matches::SIMPLE_1, *state::SIMPLE_0)
            .unwrap();
        blocks
            .increment(*transactions::SIMPLE, *matches::SIMPLE_2, *state::SIMPLE_0)
            .unwrap();
    }

    #[test]
    fn simple_8() {
        let mut blocks = Blocks::new(*AUX_8, state::SIMPLE_8.old());

        blocks
            .increment(*transactions::SIMPLE, *matches::SIMPLE_1, *state::SIMPLE_8)
            .unwrap();
        blocks
            .increment(*transactions::SIMPLE, *matches::SIMPLE_2, *state::SIMPLE_8)
            .unwrap();
    }

    #[test]
    fn simple_63() {
        let mut blocks = Blocks::new(*AUX_63, state::SIMPLE_63.old());

        blocks
            .increment(*transactions::SIMPLE, *matches::SIMPLE_1, *state::SIMPLE_63)
            .unwrap();
        blocks
            .increment(*transactions::SIMPLE, *matches::SIMPLE_2, *state::SIMPLE_63)
            .unwrap();
    }
}
