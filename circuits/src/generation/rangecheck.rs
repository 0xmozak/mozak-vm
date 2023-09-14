use std::borrow::Borrow;
use std::ops::Index;

use plonky2::hash::hash_types::RichField;

use crate::cpu::columns::CpuState;
use crate::lookup::permute_cols;
use crate::memory::columns::Memory;
use crate::rangecheck::columns::{self, RangeCheckColumnsView, MAP};
use crate::stark::mozak_stark::{Lookups, RangecheckTable, Table, TableKind};

pub(crate) const RANGE_CHECK_U16_SIZE: usize = 1 << 16;

/// Pad the rangecheck trace table to the size of 2^k rows in
/// preparation for the Halo2 lookup argument.
///
/// Note that by right the column to be checked (A) and the fixed column (S)
/// have to be extended by dummy values known to be in the fixed column if they
/// are not of size 2^k, but because our fixed column is a range from 0..2^16-1,
/// initializing our trace to all [`F::ZERO`]s takes care of this step by
/// default.
#[must_use]
fn pad_rc_trace<F: RichField>(mut trace: Vec<Vec<F>>) -> Vec<Vec<F>> {
    let len = trace[0].len().max(RANGE_CHECK_U16_SIZE).next_power_of_two();

    trace.iter_mut().for_each(move |c| c.resize(len, F::ZERO));

    trace
}

/// Converts a u32 into 2 u16 limbs represented in [`RichField`].
#[must_use]
pub fn limbs_from_u32<F: RichField>(value: u32) -> (F, F) {
    (
        F::from_noncanonical_u64((value >> 16).into()),
        F::from_noncanonical_u64((value & 0xffff).into()),
    )
}

fn push_rangecheck_row<F: RichField>(
    trace: &mut [Vec<F>],
    rangecheck_row: &[F; columns::NUM_RC_COLS],
) {
    for (i, col) in rangecheck_row.iter().enumerate() {
        trace[i].push(*col);
    }
}

pub fn extract<'a, F: RichField, V>(trace: &[V], looking_table: &Table<F>) -> Vec<F>
where
    V: Index<usize, Output = F> + 'a, {
    if let [column] = &looking_table.columns[..] {
        trace
            .iter()
            .filter(|&row| looking_table.filter_column.eval(row).is_one())
            .map(|row| column.eval(row))
            .collect()
    } else {
        panic!("Can only range check single values, not tuples.")
    }
}

/// Generates a trace table for range checks, used in building a
/// `RangeCheckStark` proof.
///
/// # Panics
///
/// Panics if:
/// 1. conversion of u32 values to u16 limbs,
/// 2. trace width does not match the number of columns,
/// 3. attempting to range check tuples instead of single values.
#[must_use]
pub fn generate_rangecheck_trace<F: RichField>(
    cpu_trace: &[CpuState<F>],
    memory_trace: &[Memory<F>],
) -> [Vec<F>; columns::NUM_RC_COLS] {
    let mut trace: Vec<Vec<F>> = vec![vec![]; columns::NUM_RC_COLS];

    for looking_table in RangecheckTable::lookups().looking_tables {
        let values = match looking_table.kind {
            TableKind::Cpu => extract(cpu_trace, &looking_table),
            TableKind::Memory => extract(memory_trace, &looking_table),
            other => unimplemented!("Can't range check {other:#?} tables"),
        };

        for val in values {
            let (limb_hi, limb_lo) = limbs_from_u32(
                u32::try_from(val.to_canonical_u64()).expect("casting value to u32 should succeed"),
            );
            let rangecheck_row = RangeCheckColumnsView {
                limb_lo,
                limb_hi,
                filter: F::ONE,
                ..Default::default()
            };
            push_rangecheck_row(&mut trace, rangecheck_row.borrow());
        }
    }

    // Pad our trace to max(RANGE_CHECK_U16_SIZE, trace[0].len())
    trace = pad_rc_trace(trace);

    // Here, we generate fixed columns for the table, used in inner table lookups.
    // We are interested in range checking 16-bit values, hence we populate with
    // values 0, 1, .., 2^16 - 1.
    trace[MAP.fixed_range_check_u16] = (0..RANGE_CHECK_U16_SIZE as u64)
        .map(F::from_noncanonical_u64)
        .collect();
    let num_rows = trace[MAP.filter].len();
    trace[MAP.fixed_range_check_u16].resize(num_rows, F::from_canonical_u64(u64::from(u16::MAX)));

    // This permutation is done in accordance to the [Halo2 lookup argument
    // spec](https://zcash.github.io/halo2/design/proving-system/lookup.html)
    let (col_input_permuted, col_table_permuted) =
        permute_cols(&trace[MAP.limb_lo], &trace[MAP.fixed_range_check_u16]);

    // We need a column for the lower limb.
    trace[MAP.limb_lo_permuted] = col_input_permuted;
    trace[MAP.fixed_range_check_u16_permuted_lo] = col_table_permuted;

    let (col_input_permuted, col_table_permuted) =
        permute_cols(&trace[MAP.limb_hi], &trace[MAP.fixed_range_check_u16]);

    // And we also need a column for the upper limb.
    trace[MAP.limb_hi_permuted] = col_input_permuted;
    trace[MAP.fixed_range_check_u16_permuted_hi] = col_table_permuted;

    trace.try_into().unwrap_or_else(|v: Vec<Vec<F>>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            columns::NUM_RC_COLS,
            v.len()
        )
    })
}

mod helpers {
    use std::collections::HashMap;

    use plonky2::hash::hash_types::RichField;

    pub fn get_skip_approx<F: RichField>(fixed_range_check_u16: &[F]) -> u16 {
        let input_as_u16_sorted: Vec<u16> = itertools::sorted(
            fixed_range_check_u16
            .iter()
            .map(|field_element| u16::try_from(field_element.to_canonical_u64()).expect("Casting to u16 should succeed"))
        ).collect();
    
    
        // ensure that first and last elements of sorted input are 0 and u16::MAX, respectively
        assert_eq!(input_as_u16_sorted[0], 0);
        assert_eq!(input_as_u16_sorted[input_as_u16_sorted.len() - 1], u16::MAX);

        let diff_vec_sorted: Vec<u16> = itertools::sorted(get_diff_values(&input_as_u16_sorted)).collect();

        let skip = main_algorithm(&diff_vec_sorted);

        skip
    }

    fn get_diff_values(input_as_u16_sorted: &[u16]) -> Vec<u16> {
        let mut diff_vec = vec![];
        let mut prev_val = input_as_u16_sorted[0];
        for val in input_as_u16_sorted.into_iter().skip(1) {
            let diff = val - prev_val;
            if diff != 0 && diff != 1 {
                diff_vec.push(diff);
            }
            prev_val = *val;
        }
        diff_vec
    }

    fn main_algorithm(diff_vec_sorted: &[u16]) -> u16 {
        let diff_vec_sorted_f64: Vec<f64> =  diff_vec_sorted.iter().map(|x| *x as f64).collect();
        let mut curr_min_heuristic_value = u16::MAX as f64;   // sum of diff never exceeds u16::MAX
        let mut curr_best_guess: u16 = 0;

        let mut prefix_sum = 0f64;
        let mut suffix_sum: f64 = diff_vec_sorted_f64.iter().sum();

        let n = diff_vec_sorted_f64.len();

        let mut reps: HashMap::<u16, f64> = HashMap::default();

        for l in 0..n { 
            let curr_val = diff_vec_sorted[l];
            *reps.entry(curr_val).or_insert(0f64) += 1f64;

            prefix_sum += diff_vec_sorted_f64[l];
            suffix_sum -= diff_vec_sorted_f64[l];

            let n_minus_l_f64 = (n - 1 - l) as f64;

            // statistical best guess. Main indended usage is do detect large jumps
            let mut heuristic_best_guess = (2f64 * suffix_sum / n_minus_l_f64).sqrt() as u32 as f64;
            let mut heuristic_value = prefix_sum + n_minus_l_f64 * (heuristic_best_guess);
            if l == (n - 1) || diff_vec_sorted_f64[l] >= heuristic_best_guess || diff_vec_sorted_f64[l+1] <= heuristic_best_guess {
                // statistical guess doesn't lie in desired range or we are at the end
                // change the guess to leftmost value of current interval
                heuristic_best_guess = diff_vec_sorted_f64[l];
                heuristic_value = prefix_sum - reps[&curr_val] * (diff_vec_sorted_f64[l] - 1f64) + suffix_sum / diff_vec_sorted_f64[l] + diff_vec_sorted_f64[l]  * n_minus_l_f64 / 2f64;
            }

            if heuristic_value < curr_min_heuristic_value {
                curr_min_heuristic_value = heuristic_value;
                curr_best_guess = heuristic_best_guess as u16;
            }
        }
        

        curr_best_guess
    }

    #[cfg(test)]
    mod tests {
        use proptest::{proptest, prop_assume};
        use proptest::prelude::ProptestConfig;
        use plonky2::field::types::Sample;

        use crate::generation::rangecheck::helpers::main_algorithm;

        fn find_sum_q_plus_r(v: &[u16], val: &u16) -> u16 {
            let mut sum = 0;
            for other_val in v.iter(){
                let q = other_val / val;
                let r = other_val % val;
                sum += q + r;
            }
            sum
        }
        fn optimal(v: &[u16]) -> u16 {
            let max = v.iter().fold(0, |acc, x| acc.max(*x));
            let mut optimal = u16::MAX;
            let mut curr_guess = 0;
            for val in 2..max+1{
                let sum = find_sum_q_plus_r(v, &val);
                if sum < optimal {
                    optimal = sum;
                    curr_guess = val;
                }
            }
            curr_guess
        }

        fn example(v: &[u16]){
            // let v = vec![4, 5, 10, 1123, 2523, 3452];
            let optimal = optimal(v);
            let alg_out = main_algorithm(v);

            let optimal_steps = find_sum_q_plus_r(&v, &optimal);
            let alg_steps =find_sum_q_plus_r(&v, &alg_out);
            println!("optimal is {} with steps {}", optimal, optimal_steps);
            println!("algorithm returns {} with steps {}", alg_out, alg_steps);
        }

        proptest!(
            #![proptest_config(ProptestConfig::with_cases(1))]
            #[test]
            fn rand_test(u: Vec<u8>) {
                prop_assume!(u.len() > 10);
                let mut v: Vec<u16> = u[..10].iter().map(|x| *x as u16).collect();
                v.sort();
                example(&v);
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use mozak_vm::instruction::{Args, Instruction, Op};
    use mozak_vm::test_utils::simple_test_code;
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;
    use crate::generation::cpu::generate_cpu_trace;
    use crate::generation::memory::generate_memory_trace;

    #[test]
    fn test_add_instruction_inserts_rangecheck() {
        type F = GoldilocksField;
        let (program, record) = simple_test_code(
            &[Instruction {
                op: Op::ADD,
                args: Args {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    ..Args::default()
                },
            }],
            // Use values that would become limbs later
            &[],
            &[(6, 0xffff), (7, 0xffff)],
        );

        let cpu_rows = generate_cpu_trace::<F>(&program, &record);
        let memory_rows = generate_memory_trace::<F>(&program, &record.executed);
        let trace = generate_rangecheck_trace::<F>(&cpu_rows, &memory_rows);

        // Check values that we are interested in
        assert_eq!(trace[MAP.filter][0], F::ONE);
        assert_eq!(trace[MAP.filter][1], F::ONE);
        assert_eq!(trace[MAP.limb_hi][0], GoldilocksField(0x0001));
        assert_eq!(trace[MAP.limb_lo][0], GoldilocksField(0xfffe));
        assert_eq!(trace[MAP.limb_lo][1], GoldilocksField(0));
    }
}
