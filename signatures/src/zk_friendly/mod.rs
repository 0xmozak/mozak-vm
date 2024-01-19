use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::{Field, Field64, PrimeField64};
use plonky2::hash::hash_types::{HashOut, HashOutTarget, RichField};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};
use plonky2::plonk::proof::ProofWithPublicInputs;

/// This is supposed to be a slice of four field
/// elements in goldilocks, since its output of
/// poseidon hash.
pub struct PublicKey {
    limbs: [u64; 4],
}

impl PublicKey {
    pub fn new(limbs: [u64; 4]) -> Option<Self> {
        match limbs
            .iter()
            .filter(|&&x| x >= GoldilocksField::ORDER)
            .count()
        {
            0 => Some(Self { limbs }),
            _ => None,
        }
    }

    pub fn get_limbs(&self) -> [u64; 4] { self.limbs }

    pub fn get_limbs_field(&self) -> [GoldilocksField; 4] {
        self.get_limbs().map(GoldilocksField::from_canonical_u64)
    }
}

impl From<HashOut<GoldilocksField>> for PublicKey {
    fn from(hash: HashOut<GoldilocksField>) -> Self {
        let limbs = hash
            .elements
            .map(|elem| GoldilocksField::to_canonical_u64(&elem));

        Self::new(limbs).unwrap()
    }
}

/// 256 bit private key
pub struct PrivateKey {
    limbs: [u8; 32],
}

impl PrivateKey {
    pub fn new(limbs: [u8; 32]) -> Self { Self { limbs } }

    pub fn get_limbs(&self) -> [u8; 32] { self.limbs }

    pub fn get_public_key(&self) -> PublicKey {
        PoseidonHash::hash_or_noop(&self.get_limbs_field()).into()
    }

    pub fn get_limbs_field(&self) -> [GoldilocksField; 32] {
        self.get_limbs().map(GoldilocksField::from_canonical_u8)
    }
}

/// For simplicity, this is assumed to be a 256 bit hash
pub struct Message {
    limbs: [u8; 32],
}

impl Message {
    pub fn new(limbs: [u8; 32]) -> Self { Self { limbs } }

    pub fn get_limbs(&self) -> [u8; 32] { self.limbs }

    pub fn get_limbs_field(&self) -> [GoldilocksField; 32] {
        self.get_limbs().map(GoldilocksField::from_canonical_u8)
    }
}

pub fn sign_circuit<F: RichField + Extendable<D>, C: GenericConfig<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    private_key_target: [Target; 32],
    public_key_target: HashOutTarget,
    msg_target: [Target; 32],
) where
    C::Hasher: AlgebraicHasher<F>, {
    // range check each limb to be 8 bits
    chain!(private_key_target, msg_target)
        .for_each(|target_limb| builder.range_check(target_limb, 8));

    // hash the private key
    let hash_private_key = builder.hash_or_noop::<C::Hasher>(private_key_target.to_vec());

    // check hash(private_key) == public key
    builder.connect_hashes(hash_private_key, public_key_target);

    // public key and msg are public inputs
    builder.register_public_inputs(&public_key_target.elements);
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
    env_logger::init();
    let mut witness = PartialWitness::<F>::new();
    let mut builder = CircuitBuilder::<F, D>::new(config);

    // create targets
    let private_key_target = builder.add_virtual_target_arr::<32>();
    let public_key_target = builder.add_virtual_target_arr::<4>();
    let msg_target = builder.add_virtual_target_arr::<32>();

    // convert inputs slices to field slices.
    let private_key_field = private_key.get_limbs().map(|x| F::from_canonical_u8(x));
    let public_key_field = public_key.get_limbs().map(|x| F::from_noncanonical_u64(x));
    let msg_field = msg.get_limbs().map(|x| F::from_canonical_u8(x));

    // set target values
    witness.set_target_arr(&private_key_target, &private_key_field);
    witness.set_target_arr(&public_key_target, &public_key_field);
    witness.set_target_arr(&msg_target, &msg_field);

    sign_circuit::<F, C, D>(
        &mut builder,
        private_key_target,
        public_key_target.into(),
        msg_target,
    );

    builder.print_gate_counts(0);
    let data = builder.build::<C>();
    let proof = data.prove(witness).unwrap();
    (data, proof)
}

#[cfg(test)]
mod tests {

    use plonky2::field::types::Sample;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use rand::Rng;

    use super::{PrivateKey, PublicKey};
    use crate::zk_friendly::Message;
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<2>>::F;

    fn generate_signature_data() -> (PrivateKey, PublicKey, Message) {
        let mut rng = rand::thread_rng();

        // generate random private key
        let private_key = PrivateKey::new(rng.gen::<[u8; 32]>());
        // get public key associated with private key
        let public_key = private_key.get_public_key();
        // generate random message
        let msg = Message::new(rng.gen::<[u8; 32]>());

        (private_key, public_key, msg)
    }

    #[test]
    fn test_signature() {
        let config = CircuitConfig::standard_recursion_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, proof) = super::prove_sign::<F, C, 2>(config, &private_key, &public_key, &msg);
        assert!(data.verify(proof).is_ok());
    }

    #[test]
    #[should_panic]
    fn test_tampering_public_key() {
        let config = CircuitConfig::standard_recursion_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, mut proof) =
            super::prove_sign::<F, C, 2>(config, &private_key, &public_key, &msg);

        // assert public key is there in public inputs
        assert_eq!(proof.public_inputs[..4], public_key.get_limbs_field());
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
            super::prove_sign::<F, C, 2>(config, &private_key, &public_key, &msg);

        // assert msg is there in public inputs
        assert_eq!(proof.public_inputs[4..], msg.get_limbs_field());
        // tamper with msg
        proof.public_inputs[4] = F::rand();
        assert!(data.verify(proof).is_ok());
    }
}
