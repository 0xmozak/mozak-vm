use std::marker::PhantomData;

use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::iop::target::Target;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::{GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::sig::{PrivateKey, PublicKey, Signature, NUM_LIMBS_U8};
use super::utils::get_hashout;
use crate::test_sig;

type ZkSigPoseidon<F, C, const D: usize> = ProofWithPublicInputs<F, C, D>;

pub struct ZkSigPoseidonSigner<F, C, const D: usize> {
    _phantom: (PhantomData<F>, PhantomData<C>),
}
impl<F, C, const D: usize> Signature<F, C, D> for ZkSigPoseidonSigner<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    type Sig = ZkSigPoseidon<F, C, D>;

    fn hash_circuit(
        _witness: &mut PartialWitness<F>,
        builder: &mut CircuitBuilder<F, D>,
        _private_key: &PrivateKey,
        _public_key: &PublicKey,
    ) -> ([Target; NUM_LIMBS_U8], [Target; NUM_LIMBS_U8]) {
        let private_key_target = builder.add_virtual_target_arr::<NUM_LIMBS_U8>();
        let public_key_target = builder.add_virtual_target_arr::<NUM_LIMBS_U8>();

        // hash the private key
        let hash_private_key = builder.hash_or_noop::<PoseidonHash>(private_key_target.to_vec());
        let public_key_as_hash = get_hashout(builder, &public_key_target);

        // check hash(private_key) == public key
        builder.connect_hashes(hash_private_key, public_key_as_hash);
        (private_key_target, public_key_target)
    }

    fn hash_private_key(private_key: &PrivateKey) -> HashOut<GoldilocksField> {
        PoseidonHash::hash_or_noop(&private_key.get_limbs_field())
    }
}

test_sig!(ZkSigPoseidonSigner);
