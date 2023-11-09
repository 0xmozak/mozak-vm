//! Module used to serialise and deserialize Stark [`AllProof`].
//! Uses Google's [flex-buffers](https://flatbuffers.dev/flexbuffers.html) to produce byte representation.

use anyhow::Result;
use flexbuffers::{FlexbufferSerializer, Reader};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use serde::{Deserialize, Serialize};

use super::proof::AllProof;

impl<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize> AllProof<F, C, D> {
    /// Serialize `AllProof` to flexbuffer.
    ///
    /// # Errors
    /// Errors if serialization fails.
    pub fn serialize_proof_to_flexbuffer(self) -> Result<FlexbufferSerializer> {
        let mut s = FlexbufferSerializer::new();
        self.serialize(&mut s)?;
        Ok(s)
    }

    /// Deserialize `AllProof` from flexbuffer.
    ///
    /// # Errors
    /// Errors if deserialization fails.
    pub fn deserialize_proof_from_flexbuffer(proof_bytes: &[u8]) -> Result<Self> {
        let r = Reader::get_root(proof_bytes)?;
        Ok(AllProof::deserialize(r)?)
    }
}

#[cfg(test)]
mod tests {
    use mozak_runner::test_utils::simple_test_code;
    use plonky2::util::timing::TimingTree;

    use crate::stark::mozak_stark::PublicInputs;
    use crate::stark::proof::AllProof;
    use crate::stark::prover::prove;
    use crate::stark::verifier::verify_proof;
    use crate::test_utils::{fast_test_config, C, D, F, S};
    use crate::utils::from_u32;

    #[test]
    fn test_serialization_deserialization() {
        let (program, record) = simple_test_code(&[], &[], &[]);
        let stark = S::default();
        let config = fast_test_config();
        let public_inputs = PublicInputs {
            entry_point: from_u32(program.entry_point),
        };

        let all_proof = prove::<F, C, D>(
            &program,
            &record,
            &stark,
            &config,
            public_inputs,
            &mut TimingTree::default(),
        )
        .unwrap();
        let s = all_proof
            .serialize_proof_to_flexbuffer()
            .expect("serialization failed");
        let all_proof_deserialized =
            AllProof::<F, C, D>::deserialize_proof_from_flexbuffer(s.view())
                .expect("deserialization failed");
        verify_proof(stark, all_proof_deserialized, &config).unwrap();
    }
}
