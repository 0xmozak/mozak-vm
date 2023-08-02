use flexbuffers::FlexbufferSerializer;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use serde::Serialize;

use super::proof::AllProof;

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> AllProof<F, C, D> {
    #[allow(dead_code)]
    fn serialize_proof_to_flexbuffer(self) -> FlexbufferSerializer {
        let mut s = flexbuffers::FlexbufferSerializer::new();
        self.serialize(&mut s).unwrap();
        s
    }
}

#[cfg(test)]
mod tests {

    use mozak_vm::test_utils::simple_test;
    use plonky2::util::timing::TimingTree;

    use crate::stark::prover::prove;
    use crate::test_utils::{standard_faster_config, C, D, F, S};
    #[test]
    fn test_serialization() {
        let record = simple_test(0, &[], &[]);
        let stark = S::default();
        let config = standard_faster_config();

        let all_proof = prove::<F, C, D>(
            &record.executed,
            &stark,
            &config,
            &mut TimingTree::default(),
        )
        .unwrap();
        let s = all_proof.serialize_proof_to_flexbuffer();
        println!("AllProof stored in {:?} bytes", s.view().len());
    }
}
