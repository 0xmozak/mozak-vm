use criterion::{criterion_group, criterion_main, Criterion};
use plonky2::plonk::circuit_data::CircuitConfig;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use rand::Rng;
use signatures::zk_friendly::keccak256::ZkSigKeccak256Signer;
use signatures::zk_friendly::poseidon::ZkSigPoseidonSigner;
use signatures::zk_friendly::sha256::ZkSigSha256Signer;
use signatures::zk_friendly::sig::{Message, PrivateKey, PublicKey, Signature, NUM_LIMBS_U8};
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<2>>::F;
const D: usize = 2;
use anyhow::Result;

fn generate_signature_data<S: Signature<F, C, D>>() -> (PrivateKey, PublicKey, Message) {
    let mut rng = rand::thread_rng();
    // generate random private key
    let private_key = PrivateKey::new(rng.gen::<[u8; NUM_LIMBS_U8]>());
    // get public key associated with private key
    let public_key: PublicKey = S::hash_private_key(&private_key).into();
    // generate random message
    let msg = Message::new(rng.gen::<[u8; NUM_LIMBS_U8]>());
    (private_key, public_key, msg)
}

fn bench_sig<S: Signature<F, C, D>>() -> Result<()> {
    type Signer = ZkSigSha256Signer<F, C, D>;
    let config = CircuitConfig::standard_recursion_zk_config();
    let (private_key, public_key, msg) = generate_signature_data::<Signer>();
    let (circuit, signature) = Signer::sign(config, &private_key, &public_key, &msg);
    circuit.verify(signature)
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("sha256", |b| {
        b.iter(|| bench_sig::<ZkSigSha256Signer<F, C, D>>())
    });
    c.bench_function("keccak256", |b| {
        b.iter(|| bench_sig::<ZkSigKeccak256Signer<F, C, D>>())
    });
    c.bench_function("poseidon", |b| {
        b.iter(|| bench_sig::<ZkSigPoseidonSigner<F, C, D>>())
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = criterion_benchmark
);
criterion_main!(benches);
