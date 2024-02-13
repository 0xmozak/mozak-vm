#[macro_export]
macro_rules! test_sig {
    ($signer: ident) => {
        #[cfg(test)]
        mod tests {
            use plonky2::field::types::Sample;
            use plonky2::plonk::circuit_data::CircuitConfig;
            use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
            use rand::Rng;

            use super::$signer;
            use crate::zk_friendly::sig::{
                Message, PrivateKey, PublicKey, Signature, NUM_LIMBS_U8,
            };
            type C = PoseidonGoldilocksConfig;
            type F = <C as GenericConfig<2>>::F;
            const D: usize = 2;
            type Signer = $signer<F, C, D>;

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
                let (circuit, mut zk_signature) =
                    Signer::sign(config, &private_key, &public_key, &msg);

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
                let (circuit, mut zk_signature) =
                    Signer::sign(config, &private_key, &public_key, &msg);

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
    };
}
