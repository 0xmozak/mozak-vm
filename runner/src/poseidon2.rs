use std::iter::repeat;

use itertools::izip;
use plonky2::hash::hash_types::RichField;
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::Poseidon2Permutation;

use crate::state::{Aux, Poseidon2Entry, Poseidon2SpongeData, State};
use crate::system::reg_abi::{REG_A1, REG_A2, REG_A3};

const NUM_HASH_OUT_ELTS: usize = 32;
/// Represents a ~256 bit hash output.
/// Each Field represent 8 bits.
#[derive(Copy, Clone, Debug)]
pub struct HashOut<F: RichField> {
    pub elements: [F; NUM_HASH_OUT_ELTS],
}

#[allow(clippy::cast_possible_truncation)]
impl<F: RichField> HashOut<F> {
    /// Each field element is converted to byte.
    pub fn to_bytes(self) -> Vec<u8> {
        self.elements
            .into_iter()
            .map(|x| x.to_canonical_u64() as u8)
            .collect()
    }
}

// Based on hash_n_to_m_no_pad() from plonky2/src/hash/hashing.rs
/// This function is sponge function which uses poseidon2 permutation function.
/// Input must be multiple of 8 bytes. It absorbs all input and the squeezes
/// 32 Field elements to generate `HashOut`.
///
///  # Panics
///
/// Panics if `PlonkyPermutation` is implemneted on `STATE_SIZE` different than
/// 12.
pub fn hash_n_to_m_with_pad<F: RichField, P: PlonkyPermutation<F>>(
    inputs: &[F],
) -> (HashOut<F>, Vec<Poseidon2SpongeData<F>>) {
    let permute_and_record_data = |perm: &mut P, sponge_data: &mut Vec<Poseidon2SpongeData<F>>| {
        let preimage: [F; 12] = perm
            .as_ref()
            .try_into()
            .expect("length must be equal to poseidon2 STATE_SIZE");
        perm.permute();
        let output = perm
            .as_ref()
            .try_into()
            .expect("length must be equal to poseidon2 STATE_SIZE");
        sponge_data.push(Poseidon2SpongeData {
            preimage,
            output,
            gen_output: F::from_bool(false),
            con_input: F::from_bool(true),
        });
    };

    let mut perm = P::new(repeat(F::ZERO));
    let inputs = inputs.to_vec();
    // input length is expected to be multiple of P::RATE
    assert!(inputs.len() % P::RATE == 0);
    let mut sponge_data = Vec::new();

    // Absorb all input chunks.
    for chunk in inputs.chunks(P::RATE) {
        perm.set_from_slice(chunk, 0);
        permute_and_record_data(&mut perm, &mut sponge_data);
    }

    // Squeeze untill we have the desired number of outputs.
    let mut outputs = Vec::new();
    loop {
        for &item in perm.squeeze() {
            outputs.push(item);
            sponge_data
                .last_mut()
                .expect("Can't fail at least one elem must be there")
                .gen_output = F::from_bool(true);
            if outputs.len() == NUM_HASH_OUT_ELTS {
                return (
                    HashOut {
                        elements: outputs.try_into().expect("can't fail"),
                    },
                    sponge_data,
                );
            }
        }
        permute_and_record_data(&mut perm, &mut sponge_data);
        sponge_data
            .last_mut()
            .expect("Can't fail at least one elem must be there")
            .con_input = F::from_bool(false);
    }
}

impl<F: RichField> State<F> {
    #[must_use]
    /// # Panics
    ///
    /// Panics if hash output of `hash_n_to_m_with_pad` has length different
    /// then expected value.
    pub fn ecall_poseidon2(self) -> (Aux<F>, Self) {
        let input_ptr = self.get_register_value(REG_A1);
        // lengths are in bytes
        let input_len = self.get_register_value(REG_A2);
        let output_ptr = self.get_register_value(REG_A3);
        let input: Vec<F> = (0..input_len)
            .map(|i| F::from_canonical_u8(self.load_u8(input_ptr + i)))
            .collect();
        let (hash, sponge_data) =
            hash_n_to_m_with_pad::<F, Poseidon2Permutation<F>>(input.as_slice());
        let hash = hash.to_bytes();
        assert!(NUM_HASH_OUT_ELTS == hash.len());
        (
            Aux {
                poseidon2: Some(Poseidon2Entry {
                    addr: input_ptr,
                    output_addr: output_ptr,
                    len: input_len.next_multiple_of(Poseidon2Permutation::RATE as u32),
                    sponge_data,
                }),
                ..Default::default()
            },
            izip!(0.., hash)
                .fold(self, |updated_self, (i, byte)| {
                    updated_self
                        .store_u8(output_ptr.wrapping_add(i), byte)
                        .unwrap()
                })
                .bump_pc(),
        )
    }
}
#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;
    use plonky2::hash::poseidon2::Poseidon2Permutation;

    #[test]
    fn test_hash_n_to_m_with_pad() {
        let data = "💥 Mozak-VM Rocks With Poseidon2";
        let mut data_bytes = data.as_bytes().to_vec();
        // VM expects input lenght to be multiple of RATE bits
        data_bytes.resize(data_bytes.len().next_multiple_of(8), 0);
        let data_fields: Vec<GoldilocksField> = data_bytes
            .iter()
            .map(|x| GoldilocksField::from_canonical_u8(*x))
            .collect();
        let (hash, _sponge_data) = super::hash_n_to_m_with_pad::<
            GoldilocksField,
            Poseidon2Permutation<GoldilocksField>,
        >(&data_fields);
        let hash_bytes = hash.to_bytes();
        assert_eq!(
            hash_bytes,
            hex_literal::hex!("4a2087727d3a040d98a37b00bddad96f6edb0fa47e0cefb1a0856b4e22a1cf91")[..]
        );
    }
}
