use anyhow::Result;
use itertools::chain;
use plonky2::field::extension::Extendable;
use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::{HashOut, RichField};
use plonky2::iop::target::Target;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, GenericHashOut};
use plonky2::plonk::proof::ProofWithPublicInputs;

pub mod keccak256;
pub mod poseidon;

/// Num of u8limbs required to hold 256 bits
pub const NUM_LIMBS_U8: usize = 32;

/// This would be hash of 256 bit long private key
pub struct PublicKey {
    limbs: [u8; NUM_LIMBS_U8],
}

impl From<HashOut<GoldilocksField>> for PublicKey {
    fn from(hash: HashOut<GoldilocksField>) -> Self {
        Self::new(hash.to_bytes().try_into().expect("should be 8 bytes long"))
    }
}

/// 256 bit private key
pub struct PrivateKey {
    limbs: [u8; NUM_LIMBS_U8],
}

/// This would be poseidon hash of the message being signed
pub struct Message {
    limbs: [u8; NUM_LIMBS_U8],
}
macro_rules! impl_limbs {
    ($i: ident) => {
        impl $i {
            pub fn new(limbs: [u8; NUM_LIMBS_U8]) -> Self { Self { limbs } }

            pub fn get_limbs(&self) -> [u8; NUM_LIMBS_U8] { self.limbs }

            pub fn get_limbs_field(&self) -> [GoldilocksField; NUM_LIMBS_U8] {
                self.get_limbs().map(GoldilocksField::from_canonical_u8)
            }
        }
    };
}

impl_limbs!(PublicKey);
impl_limbs!(PrivateKey);
impl_limbs!(Message);

pub trait Signature<F, C, const D: usize>
where
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>, {
    type Sig: From<ProofWithPublicInputs<F, C, D>> + Into<ProofWithPublicInputs<F, C, D>>;
    fn hash_private_key(private_key: &PrivateKey) -> HashOut<GoldilocksField>;
    fn hash_circuit(
        builder: &mut CircuitBuilder<F, D>,
        private_key_target: [Target; NUM_LIMBS_U8],
        public_key_target: [Target; NUM_LIMBS_U8],
    );
    fn sign(
        config: CircuitConfig,
        private_key: &PrivateKey,
        public_key: &PublicKey,
        msg: &Message,
    ) -> (CircuitData<F, C, D>, Self::Sig)
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

        // range check each limb to be 8 bits
        chain!(private_key_target, public_key_target, msg_target)
            .for_each(|target_limb| builder.range_check(target_limb, 8));

        Self::hash_circuit(&mut builder, private_key_target, public_key_target);

        // public key and msg are public inputs
        builder.register_public_inputs(&public_key_target);
        builder.register_public_inputs(&msg_target);

        builder.print_gate_counts(0);
        let circuit = builder.build::<C>();
        let proof = circuit.prove(witness).unwrap();
        (circuit, proof.into())
    }

    fn verify(circuit: CircuitData<F, C, D>, sig: Self::Sig) -> Result<()> {
        circuit.verify(sig.into())
    }
}
