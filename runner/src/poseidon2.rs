use std::iter::repeat;

use itertools::izip;
use mozak_system::system::reg_abi::{REG_A1, REG_A2, REG_A3};
use plonky2::hash::hash_types::{HashOut, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::hashing::PlonkyPermutation;
use plonky2::hash::poseidon2::Poseidon2Permutation;
use plonky2::plonk::config::GenericHashOut;

use crate::state::{Aux, Poseidon2Entry, Poseidon2SpongeData, State};

// Based on hash_n_to_m_no_pad() from plonky2/src/hash/hashing.rs
/// This function is sponge function that uses poseidon2 permutation function.
/// Input must be multiple of 8 bytes. It absorbs all input and the squeezes
/// `NUM_HASH_OUT_ELTS` Field elements to generate `HashOut`.
/// Why do we use only 4 field elements from our Poseidon2 output, but we are
/// computing 8?  (I.e. ‘rate’ is set to 8.) Technically, we could set the rate
/// to 4 (with permuting 8 -> 8). However, we (Vivek) opted for a rate of 8 is
/// because: first, it's more efficient; with each permutation, a rate of 8/12
/// (rate/width) achieves higher throughput than 4/8. Second, this approach
/// adheres to the sponge logic defined in Plonky2  
/// # Panics
///
/// Panics if `PlonkyPermutation` is implemented on `STATE_SIZE` different from
/// 12.
pub fn hash_n_to_m_no_pad<F: RichField, P: PlonkyPermutation<F>>(
    inputs: &[F],
) -> (HashOut<F>, Vec<Poseidon2SpongeData<F>>) {
    let permute_and_record_data = |perm: &mut P, sponge_data: &mut Vec<Poseidon2SpongeData<F>>| {
        // STATE_SIZE is 12 since it's hard-coded in our stark-backend
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
        // `sponge_data` is previous `perm` and `perm` permutation
        // `gen_output` is a flag that will be used later
        sponge_data.push(Poseidon2SpongeData {
            preimage,
            output,
            gen_output: F::from_bool(false),
        });
    };
    // `perm` defined in such a way that this statement will be ALWAYS true:
    // Some(F::ZERO) == perm.next()
    let mut perm = P::new(repeat(F::ZERO));
    // input length is expected to be multiple of P::RATE
    // R::RATE is 8
    assert_eq!(inputs.len() % P::RATE, 0);
    let mut sponge_data = Vec::new();

    // Absorb all input chunks.
    // Divide input-FE to chunks of P::RATE (8), so chunk = 8 FE
    for chunk in inputs.chunks(P::RATE) {
        // put `chunk` elements inside `perm` starting from index 0 - it means always
        // put at the beginning of the `perm`
        perm.set_from_slice(chunk, 0);
        // run the function that executes the permutation and append a new `sponge_data`
        // element
        permute_and_record_data(&mut perm, &mut sponge_data);
    }

    // `perm.squeeze` return P::RATE (8) elements, from these elements only first
    // NUM_HASH_OUT_ELTS (4) is taken
    let outputs: [F; NUM_HASH_OUT_ELTS] = perm.squeeze()[..NUM_HASH_OUT_ELTS]
        .try_into()
        .expect("squeeze must have minimum NUM_HASH_OUT_ELTS length");
    // set the flag for the last `sponge_data` element
    sponge_data
        .last_mut()
        .expect("Can't fail at least one elem must be there")
        .gen_output = F::from_bool(true);
    // `from` function just takes 4 elements array and create HashOut from it
    (HashOut::from(outputs), sponge_data)
}

impl<F: RichField> State<F> {
    #[must_use]
    /// # Panics
    ///
    /// Panics if hash output of `hash_n_to_m_no_pad` has length different
    /// from expected value.
    /// Note: `ecall_poseidon2` works with 3 parameters:
    /// 1) Input-Data - The data we want to hash - represented as `input_ptr`
    /// 2) Input-Data-Length - represented as `input_len`
    /// 3) Output-Hash - the expected hash value - represented as `output_ptr`
    /// Output-Hash size is constant and expected to be 32 bytes - 4 FE - 256b
    pub fn ecall_poseidon2(self) -> (Aux<F>, Self) {
        // In this step we're taking 3 `ecall_poseidon2` arguments
        let input_ptr = self.get_register_value(REG_A1);
        // lengths are in bytes
        let input_len = self.get_register_value(REG_A2);
        let output_ptr = self.get_register_value(REG_A3);
        // In this step, we're mapping one-to-one, bytes to FE
        // So if initial data is 32 bytes -> input-vector will 32 FE
        // Pay attention that `self.load8` loads from memory
        // Note: I am not sure why we map byte-to-FE and not 7 bytes to FE
        let input: Vec<F> = (0..input_len)
            .map(|i| F::from_canonical_u8(self.load_u8(input_ptr + i)))
            .collect();
        // This is the most important step, since here the actual `poseidon2` hash
        // computation's taken place. This function returns `computed` hash-value and
        // the intermediate `sponge_data`
        let (hash, sponge_data) =
            hash_n_to_m_no_pad::<F, Poseidon2Permutation<F>>(input.as_slice());
        // In this step, HashOut<F> translated to bytes. Nothing special here, since
        // internally it is just to_canonical64 and to 8 bytes. So it is just byte
        // representation. The problem is that the same preimage can give us 2 different
        // hashes, because we can add F::ORDER as in this example:
        // let x = x.to_canonical_u64();
        // x.checked_add(F::ORDER).unwrap_or(x).to_le_bytes()
        // poseidon constraints don't ensure this
        let hash = hash.to_bytes();
        assert_eq!(32, hash.len());
        // In this step, 2 things happen:
        // 1) Fill the Aux.poseidon2 entry
        // 2) Store the computed hash inside `output_ptr`
        (
            Aux {
                poseidon2: Some(Poseidon2Entry {
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
