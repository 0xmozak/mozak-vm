use plonky2::hash::hash_types::RichField;

use crate::registerinit::columns::RegisterInit;
use crate::utils::pad_trace_with_default;

/// Generates a register init ROM trace
#[must_use]
pub fn generate_register_init_trace<F: RichField>() -> Vec<RegisterInit<F>> {
    pad_trace_with_default(
        (0..32)
            .map(|i| RegisterInit {
                reg_addr: F::from_canonical_usize(i),
                value: F::ZERO,
                is_looked_up: F::from_bool(i != 0),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use plonky2::field::goldilocks_field::GoldilocksField;
    use plonky2::field::types::Field;

    use super::*;

    type F = GoldilocksField;

    #[test]
    fn test_generate_registerinit_trace() {
        let trace = generate_register_init_trace::<F>();
        assert_eq!(trace.len(), 32);
        for (i, r) in trace.iter().enumerate().take(32) {
            assert!(match i {
                0 => r.is_looked_up.is_zero(),
                _ => r.is_looked_up.is_one(),
            });
            assert_eq!(r.reg_addr, F::from_canonical_usize(i));
            assert_eq!(r.value, F::ZERO);
        }
    }
}
