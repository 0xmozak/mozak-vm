use core::fmt::Debug;

use expr::Expr;
use mozak_circuits_derive::StarkNameDisplay;

use super::columns::Register;
use crate::columns_view::{HasNamedColumns, NumberOfColumns};
use crate::expr::{ConstraintBuilder, GenerateConstraints, StarkFrom, Vars};
use crate::unstark::NoColumns;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterConstraints {}

pub type RegisterStark<F, const D: usize> =
    StarkFrom<F, RegisterConstraints, { D }, { COLUMNS }, { PUBLIC_INPUTS }>;

impl<F, const D: usize> HasNamedColumns for RegisterStark<F, D> {
    type Columns = Register<F>;
}

const COLUMNS: usize = Register::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl GenerateConstraints<{ COLUMNS }, { PUBLIC_INPUTS }> for RegisterConstraints {
    type PublicInputs<E: Debug> = NoColumns<E>;
    type View<E: Debug> = Register<E>;

    /// Constraints for the [`RegisterStark`]:
    ///
    /// 1) `is_init`, `is_read`, `is_write`, and the virtual `is_used` column
    ///    are binary columns. The `is_used` column is the sum of all the other
    ///    ops columns combined, to differentiate between real trace rows and
    ///    padding rows.
    /// 2) The virtual `is_used` column only take values 0 or 1.
    /// 3) Only rd changes.
    /// 4) Address changes only when `nv.is_init` == 1.
    /// 5) Address either stays the same or increments by 1.
    /// 6) Addresses go from 1 to 31.  Address 0 is handled by
    ///    `RegisterZeroStark`.
    ///
    /// For more details, refer to the [Notion
    /// document](https://www.notion.so/0xmozak/Register-File-STARK-62459d68aea648a0abf4e97aa0093ea2).
    fn generate_constraints<'a, T: Copy + Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>> {
        let lv = vars.local_values;
        let nv = vars.next_values;
        let mut constraints = ConstraintBuilder::default();

        // Constraint 1: filter columns take 0 or 1 values only.
        constraints.always(lv.ops.is_init.is_binary());
        constraints.always(lv.ops.is_read.is_binary());
        constraints.always(lv.ops.is_write.is_binary());
        constraints.always(lv.is_used().is_binary());

        // Constraint 2: virtual `is_used` column can only take values 0 or 1.
        // (lv.is_used() - nv.is_used() - 1) is expressed as such, because
        // lv.is_used() = 1 in the last real row, and
        // nv.is_used() = 0 in the first padding row.
        constraints.transition(nv.is_used() * (nv.is_used() - lv.is_used()));

        // Constraint 3: only rd changes.
        // We reformulate the above constraint as such:
        // For any register, only `is_write`, `is_init` or the virtual `is_used`
        // column should be able to change values of registers.
        // `is_read` should not change the values of registers.
        constraints.transition(nv.ops.is_read * (nv.value - lv.value));

        // Constraint 4: Address changes only when nv.is_init == 1.
        // We reformulate the above constraint to be:
        // if next `is_read` == 1 or next `is_write` == 1, the address cannot
        // change.
        constraints.transition((nv.ops.is_read + nv.ops.is_write) * (nv.addr - lv.addr));

        // Constraint 5: Address either stays the same or increments by 1.
        constraints.transition((nv.addr - lv.addr) * (nv.addr - lv.addr - 1));

        // Constraint 6: addresses go from 1 to 31.
        constraints.first_row(lv.addr - 1);
        constraints.last_row(lv.addr - 31);

        constraints
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use mozak_runner::code;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{reg, u32_extra};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use super::*;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = RegisterStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn test_circuit() -> Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)
    }

    fn prove_stark<Stark: ProveAndVerify>(a: u32, b: u32, imm: u32, rd: u8) {
        let (program, record) = code::execute(
            [
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd,
                        rs1: 6,
                        rs2: 7,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::ADD,
                    args: Args {
                        rd,
                        rs1: 6,
                        imm,
                        ..Args::default()
                    },
                },
            ],
            &[],
            &[(6, a), (7, b)],
        );
        Stark::prove_and_verify(&program, &record).unwrap();
    }

    use proptest::prelude::ProptestConfig;
    use proptest::proptest;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]
        #[test]
        fn prove_register(a in u32_extra(), b in u32_extra(), imm in u32_extra(), rd in reg()) {
            prove_stark::<RegisterStark<F, D>>(a, b, imm, rd);
        }
    }
}
