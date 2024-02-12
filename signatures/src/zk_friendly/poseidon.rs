use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, GenericHashOut, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

use super::{Message, PrivateKey, PublicKey, NUM_LIMBS_U8};

impl From<HashOut<GoldilocksField>> for PublicKey {
    fn from(hash: HashOut<GoldilocksField>) -> Self {
        Self::new(hash.to_bytes().try_into().expect("should be 8 bytes long"))
    }
}

impl PrivateKey {
    // todo: customize hash
    pub fn get_public_key(&self) -> PublicKey {
        PoseidonHash::hash_or_noop(&self.get_limbs_field()).into()
    }
}

pub fn sign_circuit<F: RichField + Extendable<D>, C: GenericConfig<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    private_key_target: [Target; NUM_LIMBS_U8],
    public_key_target: [Target; NUM_LIMBS_U8],
    msg_target: [Target; NUM_LIMBS_U8],
) where
    C::Hasher: AlgebraicHasher<F>, {
    // range check each limb to be 8 bits
    chain!(private_key_target, msg_target)
        .for_each(|target_limb| builder.range_check(target_limb, 8));

    // hash the private key
    let hash_private_key = builder.hash_or_noop::<C::Hasher>(private_key_target.to_vec());
    let public_key_as_hash = get_hashout(builder, &public_key_target);
    // check hash(private_key) == public key
    builder.connect_hashes(hash_private_key, public_key_as_hash);

    // public key and msg are public inputs
    builder.register_public_inputs(&public_key_target);
    builder.register_public_inputs(&msg_target);
}

pub fn prove_sign<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>(
    config: CircuitConfig,
    private_key: &PrivateKey,
    public_key: &PublicKey,
    msg: &Message,
) -> (CircuitData<F, C, D>, ProofWithPublicInputs<F, C, D>)
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut witness = PartialWitness::<F>::new();
    let mut builder = CircuitBuilder::<F, D>::new(config);

    // create targets
    let private_key_target = builder.add_virtual_target_arr::<NUM_LIMBS_U8>();
    let public_key_target = builder.add_virtual_target_arr::<NUM_LIMBS_U8>();
    let msg_target = builder.add_virtual_target_arr::<NUM_LIMBS_U8>();

    // convert inputs slices to field slices.
    let private_key_field = private_key.get_limbs().map(|x| F::from_canonical_u8(x));
    let public_key_field = public_key.get_limbs().map(|x| F::from_canonical_u8(x));
    let msg_field = msg.get_limbs().map(|x| F::from_canonical_u8(x));

    // set target values
    witness.set_target_arr(&private_key_target, &private_key_field);
    witness.set_target_arr(&public_key_target, &public_key_field);
    witness.set_target_arr(&msg_target, &msg_field);

    sign_circuit::<F, C, D>(
        &mut builder,
        private_key_target,
        public_key_target,
        msg_target,
    );

    builder.print_gate_counts(0);
    let data = builder.build::<C>();
    let proof = data.prove(witness).unwrap();
    (data, proof)
}

pub fn get_hashout<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    limbs: &[Target; 32],
) -> HashOutTarget {
    let hash_out_target = builder.add_virtual_hash();
    let zero = builder.zero();
    let base = builder.constant(F::from_canonical_u16(1 << 8));
    for i in 0..4 {
        let u32_target = limbs[8 * i..8 * i + 8]
            .iter()
            .rev()
            .fold(zero, |acc, limb| builder.mul_add(acc, base, *limb));
        builder.connect(hash_out_target.elements[i], u32_target);
    }
    hash_out_target
}

#[cfg(test)]
mod tests {

    use plonky2::field::types::Sample;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use rand::Rng;

    use super::{Message, PrivateKey, PublicKey};
    use crate::zk_friendly::NUM_LIMBS_U8;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<2>>::F;
    const D: usize = 2;

    fn generate_signature_data() -> (PrivateKey, PublicKey, Message) {
        let _ = env_logger::try_init();
        let mut rng = rand::thread_rng();

        // generate random private key
        let private_key = PrivateKey::new(rng.gen::<[u8; NUM_LIMBS_U8]>());
        // get public key associated with private key
        let public_key = private_key.get_public_key();
        // generate random message
        let msg = Message::new(rng.gen::<[u8; NUM_LIMBS_U8]>());

        (private_key, public_key, msg)
    }

    #[test]
    fn test_signature() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, proof) = super::prove_sign::<F, C, 2>(config, &private_key, &public_key, &msg);
        assert!(data.verify(proof).is_ok());
    }

    #[test]
    #[should_panic]
    fn test_tampering_public_key() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, mut proof) =
            super::prove_sign::<F, C, D>(config, &private_key, &public_key, &msg);

        // assert public key is there in public inputs
        assert_eq!(
            proof.public_inputs[..NUM_LIMBS_U8],
            public_key.get_limbs_field()
        );
        // tamper with public key
        proof.public_inputs[0] = F::rand();
        assert!(data.verify(proof).is_ok());
    }

    #[test]
    #[should_panic]
    fn test_tampering_message() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, mut proof) =
            super::prove_sign::<F, C, D>(config, &private_key, &public_key, &msg);

        // assert msg is there in public inputs
        assert_eq!(proof.public_inputs[NUM_LIMBS_U8..], msg.get_limbs_field());
        // tamper with msg
        proof.public_inputs[NUM_LIMBS_U8] = F::rand();
        assert!(data.verify(proof).is_ok());
    }
}
