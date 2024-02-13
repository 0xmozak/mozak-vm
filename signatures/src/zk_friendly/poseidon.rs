use std::marker::PhantomData;

use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::iop::target::Target;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::config::{GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{PrivateKey, PublicKey, Signature, NUM_LIMBS_U8};

pub struct ZkSigPoseidon<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    signature: ProofWithPublicInputs<F, C, D>,
}

impl<F, C, const D: usize> From<ProofWithPublicInputs<F, C, D>> for ZkSigPoseidon<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn from(signature: ProofWithPublicInputs<F, C, D>) -> Self { Self { signature } }
}

impl<F, C, const D: usize> Into<ProofWithPublicInputs<F, C, D>> for ZkSigPoseidon<F, C, D>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
{
    fn into(self) -> ProofWithPublicInputs<F, C, D> { self.signature }
}

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

pub fn get_hashout<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    limbs: &[Target; 32],
) -> HashOutTarget {
    let hash_out_target = builder.add_virtual_hash();
    let zero = builder.zero();
    let base = builder.constant(F::from_canonical_u16(1 << 8));
    for i in 0..4 {
        let u64_target = limbs[8 * i..8 * i + 8]
            .iter()
            .rev()
            .fold(zero, |acc, limb| builder.mul_add(acc, base, *limb));
        builder.connect(hash_out_target.elements[i], u64_target);
    }
    hash_out_target
}

#[cfg(test)]
mod tests {

    use plonky2::field::types::Sample;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use rand::Rng;

    use crate::zk_friendly::poseidon::ZkSigPoseidonSigner;
    use crate::zk_friendly::{Message, PrivateKey, PublicKey, Signature, NUM_LIMBS_U8};
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<2>>::F;
    const D: usize = 2;
    type Signer = ZkSigPoseidonSigner<F, C, D>;

    fn generate_signature_data() -> (PrivateKey, PublicKey, Message) {
        let _ = env_logger::try_init();
        let mut rng = rand::thread_rng();

        // generate random private key
        let private_key = PrivateKey::new(rng.gen::<[u8; NUM_LIMBS_U8]>());
        // get public key associated with private key
        let public_key = Signer::hash_private_key(&private_key).into();
        // generate random message
        let msg = Message::new(rng.gen::<[u8; NUM_LIMBS_U8]>());

        (private_key, public_key, msg)
    }

    #[test]
    fn test_signature() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (circuit, zk_signature) = Signer::sign(config, &private_key, &public_key, &msg);
        assert!(Signer::verify(circuit, zk_signature).is_ok());
    }

    #[test]
    #[should_panic]
    fn test_tampering_public_key() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (circuit, mut zk_signature) = Signer::sign(config, &private_key, &public_key, &msg);

        // assert public key is there in public inputs
        assert_eq!(
            zk_signature.signature.public_inputs[..NUM_LIMBS_U8],
            public_key.get_limbs_field()
        );
        // tamper with public key
        zk_signature.signature.public_inputs[0] = F::rand();
        assert!(Signer::verify(circuit, zk_signature).is_ok());
    }

    #[test]
    #[should_panic]
    fn test_tampering_message() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (circuit, mut zk_signature) = Signer::sign(config, &private_key, &public_key, &msg);

        // assert msg is there in public inputs
        assert_eq!(
            zk_signature.signature.public_inputs[NUM_LIMBS_U8..],
            msg.get_limbs_field()
        );
        // tamper with msg
        zk_signature.signature.public_inputs[NUM_LIMBS_U8] = F::rand();
        assert!(Signer::verify(circuit, zk_signature).is_ok());
    }
}
