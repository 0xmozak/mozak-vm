use std::iter::repeat;

use itertools::{chain, izip};
use mozak_sdk::core::constants::DIGEST_BYTES;
use mozak_sdk::core::reg_abi::{REG_A1, REG_A2, REG_A3};
use plonky2::hash::hash_types::{HashOut, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::{Poseidon2Permutation, WIDTH};
use plonky2::plonk::config::GenericHashOut;

use crate::state::{Aux, State};

#[derive(Debug, Clone, Default)]
pub struct SpongeData<F> {
    pub preimage: [F; WIDTH],
    pub output: [F; WIDTH],
    pub gen_output: F,
}

#[derive(Debug, Clone, Default)]
pub struct Entry<F: RichField> {
    pub addr: u32,
    pub output_addr: u32,
    pub len: u32,
    pub sponge_data: Vec<SpongeData<F>>,
}

// Based on hash_n_to_m_no_pad() from plonky2/src/hash/hashing.rs
/// This function is sponge function which uses poseidon2 permutation function.
/// Input must be multiple of 8 bytes. It absorbs all input and the squeezes
/// `NUM_HASH_OUT_ELTS` Field elements to generate `HashOut`.
///
///  # Panics
///
/// Panics if `PlonkyPermutation` is implemented on `STATE_SIZE` different than
/// 12.
pub fn hash_n_to_m_no_pad<F: RichField, P: PlonkyPermutation<F>>(
    inputs: &[F],
) -> (HashOut<F>, Vec<SpongeData<F>>) {
    let permute_and_record_data = |perm: &mut P, sponge_data: &mut Vec<SpongeData<F>>| {
        const STATE_SIZE: usize = 12;
        assert_eq!(STATE_SIZE, P::WIDTH);
        let preimage: [F; STATE_SIZE] = perm
            .as_ref()
            .try_into()
            .expect("length must be equal to poseidon2 STATE_SIZE");
        perm.permute();
        let output = perm
            .as_ref()
            .try_into()
            .expect("length must be equal to poseidon2 STATE_SIZE");
        sponge_data.push(SpongeData {
            preimage,
            output,
            gen_output: F::from_bool(false),
        });
    };

    let mut perm = P::new(repeat(F::ZERO));
    // input length is expected to be multiple of P::RATE
    assert_eq!(inputs.len() % P::RATE, 0);
    let mut sponge_data = Vec::new();

    // Absorb all input chunks.
    for chunk in inputs.chunks(P::RATE) {
        perm.set_from_slice(chunk, 0);
        permute_and_record_data(&mut perm, &mut sponge_data);
    }

    let outputs: [F; NUM_HASH_OUT_ELTS] = perm.squeeze()[..NUM_HASH_OUT_ELTS]
        .try_into()
        .expect("squeeze must have minimum NUM_HASH_OUT_ELTS length");
    sponge_data
        .last_mut()
        .expect("Can't fail at least one elem must be there")
        .gen_output = F::from_bool(true);
    (HashOut::from(outputs), sponge_data)
}

impl<F: RichField> State<F> {
    #[must_use]
    /// # Panics
    ///
    /// Panics if hash output of `hash_n_to_m_no_pad` has length different
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
            hash_n_to_m_no_pad::<F, Poseidon2Permutation<F>>(input.as_slice());
        let hash = hash.to_bytes();
        assert_eq!(DIGEST_BYTES, hash.len());

        let mem_addresses_used: Vec<u32> = chain!(
            (0..input_len).map(|i| input_ptr.wrapping_add(i)),
            izip!(0.., &hash).map(|(i, _)| output_ptr.wrapping_add(i))
        )
        .collect();
        (
            Aux {
                mem_addresses_used,
                poseidon2: Some(Entry {
                    addr: input_ptr,
                    output_addr: output_ptr,
                    len: input_len.next_multiple_of(
                        u32::try_from(Poseidon2Permutation::<F>::RATE).expect("RATE > 2^32"),
                    ),
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
    use plonky2::hash::hashing::PlonkyPermutation;
    use plonky2::hash::poseidon2::{Poseidon2Hash, Poseidon2Permutation};
    use plonky2::plonk::config::{GenericHashOut, Hasher};

    #[test]
    fn test_hash_n_to_m_no_pad() {
        let data = "💥 Mozak-VM Rocks With Poseidon2";
        let mut data_bytes = data.as_bytes().to_vec();
        // VM expects input length to be multiple of RATE
        data_bytes.resize(
            data_bytes
                .len()
                .next_multiple_of(Poseidon2Permutation::<GoldilocksField>::RATE),
            0,
        );
        let data_fields: Vec<GoldilocksField> = data_bytes
            .iter()
            .map(|x| GoldilocksField::from_canonical_u8(*x))
            .collect();
        let (hash, _sponge_data) = super::hash_n_to_m_no_pad::<
            GoldilocksField,
            Poseidon2Permutation<GoldilocksField>,
        >(&data_fields);
        let hash_bytes = hash.to_bytes();
        assert_eq!(
            hash_bytes,
            Poseidon2Hash::hash_no_pad(&data_fields).to_bytes()
        );
    }
}
