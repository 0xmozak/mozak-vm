use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::GenericConfig;

pub mod summarized;
pub mod unpruned;

pub trait CircuitType {
    type PublicIndices;
    type LeafSubCircuit: SubCircuit<Self::PublicIndices>;
    type BranchSubCircuit<'a, const D: usize>: SubCircuit<Self::PublicIndices>;
}

pub trait Circuit<CT, F, C, const D: usize>
where
    CT: CircuitType,
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    fn sub_circuit(&self) -> &dyn SubCircuit<CT::PublicIndices>;
    fn circuit_data(&self) -> &CircuitData<F, C, D>;
}

pub trait SubCircuit<PublicIndices> {
    fn pis(&self) -> usize;
    fn get_indices(&self) -> PublicIndices;
}

pub trait LeafCircuit<CT, F, C, const D: usize>: Circuit<CT, F, C, D>
where
    CT: CircuitType,
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    fn leaf_sub_circuit(&self) -> &CT::LeafSubCircuit;
}

pub trait BranchCircuit<CT, F, C, const D: usize>: Circuit<CT, F, C, D>
where
    CT: CircuitType,
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    fn branch_sub_circuit(&self) -> &CT::BranchSubCircuit<'_, D>;
}
