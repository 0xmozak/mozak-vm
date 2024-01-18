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
}

impl From<HashOut<GoldilocksField>> for PublicKey {
    fn from(hash: HashOut<GoldilocksField>) -> Self {
        let mut limbs: [u64; 4] = [0; 4];
        for i in 0..4 {
            limbs[i] = hash.elements[i].to_canonical_u64();
        }
        Self { limbs }
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
        let limbs_field: Vec<GoldilocksField> = self
            .get_limbs()
            .iter()
            .map(|limb| GoldilocksField::from_canonical_u8(*limb))
            .collect();
        PoseidonHash::hash_or_noop(&limbs_field).into()
    }
}
pub struct Message([u8; 32]);

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

    let hash_private_key = builder.hash_or_noop::<C::Hasher>(private_key_target.to_vec());

    // check hash(private_key) == public key
    builder.connect_hashes(hash_private_key, public_key_target);

    // public key and msg are public inputs
    builder.register_public_inputs(&public_key_target.elements);
    builder.register_public_inputs(&msg_target);
}

pub fn prove_sign<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>(
    config: CircuitConfig,
    private_key: PrivateKey,
    public_key: PublicKey,
    msg: Message,
) -> (CircuitData<F, C, D>, ProofWithPublicInputs<F, C, D>)
where
    C::Hasher: AlgebraicHasher<F>, {
    let mut witness = PartialWitness::<F>::new();
    let mut builder = CircuitBuilder::<F, D>::new(config);

    let private_key_target = builder.add_virtual_target_arr::<32>();
    let public_key_target = builder.add_virtual_target_arr::<4>();
    let msg_target = builder.add_virtual_target_arr::<32>();

    let private_key_field = private_key.get_limbs().map(|x| F::from_canonical_u8(x));
    let public_key_field = public_key.get_limbs().map(|x| F::from_canonical_u64(x));
    let msg_field = msg.0.map(|x| F::from_canonical_u8(x));

    witness.set_target_arr(&private_key_target, &private_key_field);
    witness.set_target_arr(&public_key_target, &public_key_field);
    witness.set_target_arr(&msg_target, &msg_field);

    sign_circuit::<F, C, D>(
        &mut builder,
        private_key_target,
        public_key_target.into(),
        msg_target,
    );

    let data = builder.build::<C>();
    let proof = data.prove(witness).unwrap();
    (data, proof)
}

#[cfg(test)]
mod tests {
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

    use super::PrivateKey;
    use crate::zk_friendly::Message;

    #[test]
    fn test_signature() {
        let config = CircuitConfig::standard_recursion_config();
        type C = PoseidonGoldilocksConfig;
        type F = <C as GenericConfig<2>>::F;
        let private_key = PrivateKey { limbs: [1; 32] };
        let public_key = private_key.get_public_key();

        let msg = Message([1; 32]);

        let (data, proof) = super::prove_sign::<F, C, 2>(config, private_key, public_key, msg);
        println!("proof size: {}", proof.public_inputs.len());
        assert!(data.verify(proof).is_ok());
    }
}
