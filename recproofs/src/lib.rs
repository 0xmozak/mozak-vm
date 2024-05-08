use std::iter::zip;

use enumflags2::{bitflags, BitFlags};
use iter_fixed::IntoIteratorFixed;
use itertools::{chain, Itertools};
use plonky2::field::extension::Extendable;
use plonky2::gates::noop::NoopGate;
use plonky2::hash::hash_types::{
    HashOut, HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS,
};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{
    CircuitConfig, CircuitData, CommonCircuitData, VerifierCircuitTarget,
};
use plonky2::plonk::config::{AlgebraicHasher, GenericConfig, Hasher};

pub mod circuits;
pub mod indices;
pub mod subcircuits;

#[cfg(any(feature = "test", test))]
pub mod test_utils {
    use itertools::{chain, Itertools};
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::{HashOut, RichField};
    use plonky2::hash::poseidon2::Poseidon2Hash;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, Hasher, Poseidon2GoldilocksConfig};

    #[must_use]
    const fn fast_test_circuit_config() -> CircuitConfig {
        let mut config = CircuitConfig::standard_recursion_config();
        config.security_bits = 1;
        config.num_challenges = 1;
        config.fri_config.cap_height = 0;
        config.fri_config.proof_of_work_bits = 0;
        config.fri_config.num_query_rounds = 1;
        config
    }

    pub const CONFIG: CircuitConfig = fast_test_circuit_config();

    pub fn hash_str(v: &str) -> HashOut<F> {
        let v: Vec<_> = v.bytes().map(F::from_canonical_u8).collect();
        Poseidon2Hash::hash_no_pad(&v)
    }

    pub fn hash_branch<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
        let [l0, l1, l2, l3] = left.elements;
        let [r0, r1, r2, r3] = right.elements;
        Poseidon2Hash::hash_no_pad(&[l0, l1, l2, l3, r0, r1, r2, r3])
    }

    pub fn hash_branch_bytes<F: RichField>(left: &HashOut<F>, right: &HashOut<F>) -> HashOut<F> {
        let bytes = chain!(left.elements, right.elements)
            .flat_map(|v| v.to_canonical_u64().to_le_bytes())
            .map(|v| F::from_canonical_u8(v))
            .collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }

    pub const D: usize = 2;
    pub type C = Poseidon2GoldilocksConfig;
    pub type F = <C as GenericConfig<D>>::F;
    pub const fn make_fs<const N: usize>(vs: [u64; N]) -> [F; N] {
        let mut f = [F::ZERO; N];
        let mut i = 0;
        while i < N {
            f[i] = GoldilocksField(vs[i]);
            i += 1;
        }
        f
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EventType {
    Write = 0,
    Ensure = 1,
    Read = 2,
    GiveOwner = 3,
    TakeOwner = 4,
    CreditDelta = 5,
}

impl EventType {
    fn constant<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> Target
    where
        F: RichField + Extendable<D>, {
        builder.constant(F::from_canonical_u64(self as u64))
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EventFlags {
    WriteFlag = 1 << 0,
    EnsureFlag = 1 << 1,
    ReadFlag = 1 << 2,
    GiveOwnerFlag = 1 << 3,
    TakeOwnerFlag = 1 << 4,
}

impl EventFlags {
    fn count() -> usize { BitFlags::<Self>::ALL.len() }

    fn index(self) -> usize { (self as u8).trailing_zeros() as usize }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Event<F> {
    owner: [F; 4],
    ty: EventType,
    address: u64,
    value: [F; 4],
}

impl<F: RichField> Event<F> {
    pub fn bytes(self) -> impl Iterator<Item = F> {
        chain!(
            self.owner,
            [self.ty as u64, self.address].map(F::from_canonical_u64),
            self.value
        )
    }

    pub fn vm_bytes(self) -> impl Iterator<Item = F> {
        chain!(
            [self.ty as u64, self.address].map(F::from_canonical_u64),
            self.value
        )
    }

    pub fn hash(self) -> HashOut<F> {
        let bytes = self.bytes().collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }

    pub fn byte_wise_hash(self) -> HashOut<F> {
        let bytes = self
            .vm_bytes()
            .flat_map(|v| v.to_canonical_u64().to_le_bytes())
            .map(|v| F::from_canonical_u8(v))
            .collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }
}

/// Computes `if b { h0 } else { h1 }`.
pub(crate) fn select_hash<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    HashOutTarget {
        elements: core::array::from_fn(|i| builder.select(b, h0.elements[i], h1.elements[i])),
    }
}

/// Computes `if b { cap0 } else { cap1 }`.
pub(crate) fn select_cap<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    cap0: &MerkleCapTarget,
    cap1: &MerkleCapTarget,
) -> MerkleCapTarget
where
    F: RichField + Extendable<D>, {
    assert_eq!(cap0.0.len(), cap1.0.len());
    MerkleCapTarget(
        cap0.0
            .iter()
            .zip_eq(&cap1.0)
            .map(|(h0, h1)| select_hash(builder, b, *h0, *h1))
            .collect(),
    )
}

/// Computes `if b { v0 } else { v1 }`.
pub(crate) fn select_verifier<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    v0: &VerifierCircuitTarget,
    v1: &VerifierCircuitTarget,
) -> VerifierCircuitTarget
where
    F: RichField + Extendable<D>, {
    VerifierCircuitTarget {
        constants_sigmas_cap: select_cap(
            builder,
            b,
            &v0.constants_sigmas_cap,
            &v1.constants_sigmas_cap,
        ),
        circuit_digest: select_hash(builder, b, v0.circuit_digest, v1.circuit_digest),
    }
}

/// Reduce a hash-sized group of booleans by `&&`ing them together
fn and_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.and(bools[0], bools[1]),
        builder.and(bools[2], bools[3]),
    ];
    builder.and(bools[0], bools[1])
}

/// Reduce a hash-sized group of booleans by `||`ing them together
fn or_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.or(bools[0], bools[1]),
        builder.or(bools[2], bools[3]),
    ];
    builder.or(bools[0], bools[1])
}

/// Computes `a == b`.
fn are_equal<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    a: [Target; 4],
    b: [Target; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let eq = a
        .into_iter_fixed()
        .zip(b)
        .map(|(h0, h1)| builder.is_equal(h0, h1))
        .collect();
    and_helper(builder, eq)
}

/// Computes `h0 == h1`.
fn hashes_equal<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    are_equal(builder, h0.elements, h1.elements)
}

/// Computes `h0 != ZERO`.
fn hash_is_nonzero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: impl Into<HashOutTarget>,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let zero = h0
        .into()
        .elements
        .into_iter_fixed()
        .map(|h0| builder.is_nonzero(h0))
        .collect();
    // If any elements are non-zero, then it's non-zero
    or_helper(builder, zero)
}

/// Computes `a == ZERO`.
fn are_zero<F, const D: usize>(builder: &mut CircuitBuilder<F, D>, a: [Target; 4]) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let zero = a
        .into_iter_fixed()
        .map(|h0| {
            let non_zero = builder.is_nonzero(h0);
            builder.not(non_zero)
        })
        .collect();
    // All numbers must be zero to be zero
    and_helper(builder, zero)
}

/// Computes `h0 == ZERO`.
pub fn hash_is_zero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    are_zero(builder, h0.elements)
}

/// Hash left and right together if both are present, otherwise forward one
fn hash_or_forward<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    left_present: BoolTarget,
    left: [Target; NUM_HASH_OUT_ELTS],
    right_present: BoolTarget,
    right: [Target; NUM_HASH_OUT_ELTS],
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    let both_present = builder.and(left_present, right_present);

    // Construct the hash of [left, right]
    let hash_both =
        builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(left.into_iter().chain(right).collect());

    // Construct the forwarding "hash".
    let hash_absent = left
        .into_iter_fixed()
        .zip(right)
        // Since absent sides will be zero, we can just sum.
        .map(|(l, r)| builder.add(l, r))
        .collect();
    let hash_absent = HashOutTarget {
        elements: hash_absent,
    };

    // Select the hash based on presence
    select_hash(builder, both_present, hash_both, hash_absent)
}

/// Guarantee at least one `BoolTarget` is `true`.
/// Does nothing if no targets are provided
fn at_least_one_true<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    targets: impl IntoIterator<Item = BoolTarget>,
) where
    F: RichField + Extendable<D>, {
    let mut targets = targets.into_iter();
    let Some(first) = targets.next() else { return };

    // Sum all the booleans
    let total = targets.fold(first.target, |total, i| builder.add(total, i.target));

    // If all booleans were 0, self-division will be unsatisfiable
    builder.div(total, total);
}

/// Connects `x` to `v` if `maybe_v` is true
fn maybe_connect<F: RichField + Extendable<D>, const D: usize, const N: usize>(
    builder: &mut CircuitBuilder<F, D>,
    x: [Target; N],
    maybe_v: BoolTarget,
    v: [Target; N],
) {
    // Loop over the limbs
    for (parent, child) in zip(x, v) {
        let child = builder.select(maybe_v, child, parent);
        builder.connect(parent, child);
    }
}

/// Connects `x` to `y`
fn connect_arrays<F: RichField + Extendable<D>, const D: usize, const N: usize>(
    builder: &mut CircuitBuilder<F, D>,
    x: [Target; N],
    y: [Target; N],
) {
    // Loop over the limbs
    for (x, y) in zip(x, y) {
        builder.connect(x, y);
    }
}

fn hash_event<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    owner: [Target; 4],
    ty: Target,
    address: Target,
    value: [Target; 4],
) -> HashOutTarget {
    builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(chain!(owner, [ty, address], value,).collect())
}

fn byte_wise_hash_event<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    ty: Target,
    address: Target,
    value: [Target; 4],
) -> HashOutTarget {
    byte_wise_hash(builder, chain!([ty, address], value).collect())
}

fn split_bytes<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    mut source: Target,
) -> [Target; 8] {
    [(); 8]
        .into_iter_fixed()
        .enumerate()
        .map(|(i, ())| {
            if i == 7 {
                source
            } else {
                let (lo, rest) = builder.split_low_high(source, 8, 64 - 8 * i);
                source = rest;
                lo
            }
        })
        .collect()
}

fn byte_wise_hash<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    inputs: Vec<Target>,
) -> HashOutTarget {
    let bytes = inputs
        .into_iter()
        .flat_map(|v| split_bytes(builder, v))
        .collect();
    builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(bytes)
}

// Generates `CircuitData` usable for recursion.
#[must_use]
pub fn circuit_data_for_recursion<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    const D: usize,
>(
    config: &CircuitConfig,
    target_degree_bits: usize,
    public_input_size: usize,
) -> CircuitData<F, C, D>
where
    C::Hasher: AlgebraicHasher<F>, {
    // Generate a simple circuit that will be recursively verified in the out
    // circuit.
    let common = {
        let mut builder = CircuitBuilder::<F, D>::new(config.clone());
        while builder.num_gates() < 1 << 5 {
            builder.add_gate(NoopGate, vec![]);
        }
        builder.build::<C>().common
    };

    let mut builder = CircuitBuilder::<F, D>::new(config.clone());
    let proof = builder.add_virtual_proof_with_pis(&common);
    let verifier_data = builder.add_virtual_verifier_data(common.config.fri_config.cap_height);
    builder.verify_proof::<C>(&proof, &verifier_data, &common);
    for _ in 0..public_input_size {
        builder.add_virtual_public_input();
    }
    // We don't want to pad all the way up to 2^target_degree_bits, as the builder
    // will add a few special gates afterward. So just pad to
    // 2^(target_degree_bits - 1) + 1. Then the builder will pad to the next
    // power of two.
    let min_gates = (1 << (target_degree_bits - 1)) + 1;
    while builder.num_gates() < min_gates {
        builder.add_gate(NoopGate, vec![]);
    }
    builder.build::<C>()
}

/// Generate a circuit matching a given `CommonCircuitData`.
#[must_use]
pub fn dummy_circuit<F: RichField + Extendable<D>, C: GenericConfig<D, F = F>, const D: usize>(
    common_data: &CommonCircuitData<F, D>,
    register_public_inputs: impl FnOnce(&mut CircuitBuilder<F, D>),
) -> CircuitData<F, C, D> {
    let config = common_data.config.clone();

    let mut builder = CircuitBuilder::<F, D>::new(config);
    // Build up enough wires to cover all our inputs
    for _ in 0..common_data.num_public_inputs {
        let _ = builder.add_virtual_target();
    }
    register_public_inputs(&mut builder);
    while builder.num_public_inputs() < common_data.num_public_inputs {
        builder.add_virtual_public_input();
    }
    for gate in &common_data.gates {
        builder.add_gate_to_gate_set(gate.clone());
    }

    // We don't want to pad all the way up to 2^target_degree_bits, as the builder
    // will add a few special gates afterward. So just pad to
    // 2^(degree - 1) + 1. Then the builder will pad to the next
    // power of two.
    let min_gates = (1 << (common_data.degree_bits() - 1)) + 1;
    while builder.num_gates() < min_gates {
        builder.add_gate(NoopGate, vec![]);
    }

    let circuit = builder.build::<C>();
    assert_eq!(&circuit.common, common_data);
    circuit
}
