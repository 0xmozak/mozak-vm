use log;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::Target;
use plonky2::iop::witness::PartialWitness;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;
use plonky2::util::timing::TimingTree;

use crate::gadgets::hash::keccak256::{CircuitBuilderHashKeccak, WitnessHashKeccak, KECCAK256_R};
use crate::gadgets::hash::CircuitBuilderHash;
use crate::gadgets::u32::arithmetic_u32::CircuitBuilderU32;
use crate::gadgets::u32::witness::WitnessU32;

/// Number of u8 elements needed to represent a private key.
const PRIVATE_KEY_U8LIMBS: usize = 32;
/// Number of u8 elements needed to represent a public key.
const PUBLIC_KEY_U8LIMBS: usize = 32;
/// Number of u32 elements needed to represent a message.
const MESSAGE_U32LIMBS: usize = 8;

/// This would be Keccak256 hash of 256 bit long private key
pub struct PublicKey([u8; PUBLIC_KEY_U8LIMBS]);

/// 256 bit private key
pub struct PrivateKey([u8; PRIVATE_KEY_U8LIMBS]);

/// Message currently assumed to be at most 256 bits long.
pub struct Message([u32; MESSAGE_U32LIMBS]);

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

    // set private key target. Block size is 1 since 256 bits fit within a block of
    // size
    let private_key_target = builder.add_virtual_hash_input_target(1, KECCAK256_R);
    // set public key target to be hash of private key
    let public_key_target = builder.hash_keccak256(&private_key_target);

    // set witnesses accordingly
    witness.set_keccak256_input_target(&private_key_target, &private_key.0);
    witness.set_keccak256_output_target(&public_key_target, &public_key.0);

    // set message target
    let message_target = builder.add_virtual_u32_targets(MESSAGE_U32LIMBS);
    (0..MESSAGE_U32LIMBS).for_each(|i| witness.set_u32_target(message_target[i], msg.0[i]));

    // register public key as public input
    builder.register_public_inputs(
        &public_key_target
            .limbs
            .iter()
            .map(|target| target.0)
            .collect::<Vec<Target>>(),
    );

    // register message as public input
    builder.register_public_inputs(
        &message_target
            .iter()
            .map(|target| target.0)
            .collect::<Vec<Target>>(),
    );

    builder.print_gate_counts(0);
    let data = builder.build::<C>();
    let timing = TimingTree::new("prove", log::Level::Debug);
    let proof = data.prove(witness).unwrap();
    timing.print();
    (data, proof)
}

#[cfg(test)]
mod tests {

    use num::BigUint;
    use plonky2::field::types::{Field, Sample};
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use rand::Rng;
    use sha3::{Digest, Keccak256};

    use super::{Message, PrivateKey, PublicKey, MESSAGE_U32LIMBS, PRIVATE_KEY_U8LIMBS};
    type C = PoseidonGoldilocksConfig;
    type F = <C as GenericConfig<2>>::F;
    const D: usize = 2;
    /// Number of u32 limbs needed to hold a public key
    const PUBLIC_KEY_LEN_U32LIMBS: usize = 8;

    fn generate_signature_data() -> (PrivateKey, PublicKey, Message) {
        let _ = env_logger::try_init();
        let mut rng = rand::thread_rng();

        // generate random private key
        let private_key = PrivateKey(rng.gen::<[u8; PRIVATE_KEY_U8LIMBS]>());

        // set public key to be hash of private key
        let mut hasher = Keccak256::new();
        hasher.update(private_key.0);
        let result = hasher.finalize();
        let public_key = PublicKey(result.into());

        // generate random message
        let msg = Message(rng.gen::<[u32; MESSAGE_U32LIMBS]>());

        (private_key, public_key, msg)
    }

    #[test]
    fn test_signature() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, proof) = super::prove_sign::<F, C, D>(config, &private_key, &public_key, &msg);
        println!("{}", proof.public_inputs.len());
        let public_key_as_u32_vec = BigUint::from_bytes_le(&public_key.0).to_u32_digits();
        assert_eq!(
            proof.public_inputs[..PUBLIC_KEY_LEN_U32LIMBS].to_vec(),
            public_key_as_u32_vec
                .into_iter()
                .map(F::from_canonical_u32)
                .collect::<Vec<F>>()
        );
        assert_eq!(
            proof.public_inputs[PUBLIC_KEY_LEN_U32LIMBS..],
            msg.0.map(F::from_canonical_u32)
        );
        assert!(data.verify(proof).is_ok());
    }

    #[test]
    #[should_panic]
    fn test_tampering_public_key() {
        let config = CircuitConfig::standard_recursion_zk_config();
        let (private_key, public_key, msg) = generate_signature_data();
        let (data, mut proof) =
            super::prove_sign::<F, C, D>(config, &private_key, &public_key, &msg);

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

        // tamper with msg
        proof.public_inputs[PUBLIC_KEY_LEN_U32LIMBS] = F::rand();
        assert!(data.verify(proof).is_ok());
    }
}
