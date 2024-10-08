use std::marker::PhantomData;

use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::target::Target;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::{GenericConfig, GenericHashOut};
use plonky2::plonk::proof::ProofWithPublicInputs;
use plonky2_crypto::hash::keccak256::{CircuitBuilderHashKeccak, WitnessHashKeccak, KECCAK256_R};
use plonky2_crypto::hash::CircuitBuilderHash;
use sha3::{Digest, Keccak256};

use super::sig::{PrivateKey, PublicKey, Signature, NUM_LIMBS_U8};
use super::utils::biguint_le_u32_target_to_le_u8_target;
use crate::test_sig;

type ZkSigKeccak256<F, C, const D: usize> = ProofWithPublicInputs<F, C, D>;

pub struct ZkSigKeccak256Signer<F, C, const D: usize> {
    _phantom: (PhantomData<F>, PhantomData<C>),
}
impl<F, C, const D: usize> Signature<F, C, D> for ZkSigKeccak256Signer<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    type Sig = ZkSigKeccak256<F, C, D>;

    fn hash_circuit(
        witness: &mut PartialWitness<F>,
        builder: &mut CircuitBuilder<F, D>,
        private_key: &PrivateKey,
        public_key: &PublicKey,
    ) -> ([Target; NUM_LIMBS_U8], [Target; NUM_LIMBS_U8]) {
        // set private key target. Block size is 1 since 256 bits fit within a block of
        // size
        let private_key_target = builder.add_virtual_hash_input_target(1, KECCAK256_R);
        // set public key target to be hash of private key
        let public_key_target = builder.hash_keccak256(&private_key_target);

        // set witnesses accordingly
        witness.set_keccak256_input_target(&private_key_target, &private_key.get_limbs());
        witness.set_keccak256_output_target(&public_key_target, &public_key.get_limbs());

        let public_key_target_u8 = biguint_le_u32_target_to_le_u8_target(
            builder,
            public_key_target
                .limbs
                .to_vec()
                .as_slice()
                .try_into()
                .expect("hash should have 8 u32 limbs"),
        );
        let private_key_target_u8 = biguint_le_u32_target_to_le_u8_target(
            builder,
            private_key_target.input.limbs.to_vec().as_slice()[..8]
                .try_into()
                .expect("private key should have 8 u32 limbs"),
        );

        (private_key_target_u8, public_key_target_u8)
    }

    fn hash_private_key(private_key: &PrivateKey) -> HashOut<GoldilocksField> {
        let mut hasher = Keccak256::new();
        hasher.update(private_key.get_limbs());
        let result = hasher.finalize();
        HashOut::from_bytes(&result)
    }
}
test_sig!(ZkSigKeccak256Signer);
