use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;

use crate::bitshift::columns::Bitshift;
use crate::bitwise::columns::XorView;
use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::program::columns::ProgramRom;
use crate::stark::mozak_stark::{CpuTable, Table};

columns_view_impl!(OpSelectors);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct OpSelectors<T> {
    pub add: T,
    pub sub: T,
    pub xor: T,
    pub or: T,
    pub and: T,
    pub divu: T,
    pub remu: T,
    pub mul: T,
    pub mulhu: T,
    pub sll: T,
    pub slt: T,
    pub sltu: T,
    pub srl: T,
    pub jalr: T,
    pub beq: T,
    pub bne: T,
    pub sb: T,
    pub lbu: T,
    pub blt: T,
    pub bltu: T,
    pub bge: T,
    pub bgeu: T,
    pub ecall: T,
}

columns_view_impl!(Instruction);
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Instruction<T> {
    /// The original instruction (+ imm_value) used for program
    /// cross-table-lookup.
    pub pc: T,

    pub ops: OpSelectors<T>,
    pub rs1_select: [T; 32],
    pub rs2_select: [T; 32],
    pub rd_select: [T; 32],
    pub imm_value: T,
    pub branch_target: T,
}

columns_view_impl!(CpuState);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuState<T> {
    pub clk: T,
    pub inst: Instruction<T>,

    pub halted: T,

    pub op1_value: T,
    // The sum of the value of the second operand register and the
    // immediate value. Wrapped around to fit in a `u32`.
    pub op2_value: T,
    /// The sum of the value of the second operand
    /// register and the immediate value with possible overflow, ie summed as
    /// field elements in a 64-bit field.
    pub op2_value_overflowing: T,
    pub dst_value: T,

    pub regs: [T; 32],

    // 0 mean non-negative, 1 means negative.
    pub op1_sign_bit: T,
    pub op2_sign_bit: T,

    // TODO: range check
    pub abs_diff: T,
    pub cmp_diff_inv: T,
    pub less_than: T,
    // If `op_diff == 0`, then `not_diff == 1`, else `not_diff == 0`.
    // We only need this intermediate variable to keep the constraint degree <= 3.
    pub not_diff: T,

    pub xor: XorView<T>,

    pub bitshift: Bitshift<T>,

    pub quotient: T,
    pub remainder: T,
    pub remainder_slack: T,
    pub divisor_inv: T,
    pub divisor: T,

    pub multiplier: T,
    pub product_low_bits: T,
    pub product_high_bits: T,
    pub product_high_diff_inv: T,
}

make_col_map!(CpuColumnsExtended);
columns_view_impl!(CpuColumnsExtended);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuColumnsExtended<T> {
    pub cpu: CpuState<T>,
    pub permuted: ProgramRom<T>,
}

pub const NUM_CPU_COLS: usize = CpuState::<()>::NUMBER_OF_COLUMNS;

impl<T: PackedField> CpuState<T> {
    #[must_use]
    pub fn shifted(places: u64) -> T::Scalar { T::Scalar::from_canonical_u64(1 << places) }

    pub fn op_diff(&self) -> T { self.op1_value - self.op2_value }

    // TODO(Matthias): unify where we specify `is_signed` for constraints and trace
    // generation. Also, later, take mixed sign (for MULHSU) into account.
    pub fn is_signed(&self) -> T { self.inst.ops.slt + self.inst.ops.bge + self.inst.ops.blt }

    /// Value of the first operand, as if converted to i64.
    ///
    /// For unsigned operations: `Field::from_noncanonical_i64(op1 as i64)`
    /// For signed operations: `Field::from_noncanonical_i64(op1 as i32 as i64)`
    ///
    /// So range is `i32::MIN..=u32::MAX`
    pub fn op1_full_range(&self) -> T { self.op1_value - self.op1_sign_bit * Self::shifted(32) }

    /// Value of the second operand, as if converted to i64.
    ///
    /// So range is `i32::MIN..=u32::MAX`
    pub fn op2_full_range(&self) -> T { self.op2_value - self.op2_sign_bit * Self::shifted(32) }

    pub fn signed_diff(&self) -> T { self.op1_full_range() - self.op2_full_range() }
}

/// Expressions we need to range check
///
/// Currently, we only support expressions over the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
    let ops = &MAP.cpu.inst.ops;
    vec![
        CpuTable::new(
            Column::singles([MAP.cpu.dst_value]),
            Column::single(ops.add),
        ),
        CpuTable::new(
            Column::singles([MAP.cpu.abs_diff]),
            Column::many([ops.bge, ops.blt]),
        ),
        CpuTable::new(
            Column::singles([MAP.cpu.product_high_bits]),
            Column::many([ops.mul, ops.mulhu]),
        ),
        CpuTable::new(
            Column::singles([MAP.cpu.product_low_bits]),
            Column::many([ops.mul, ops.mulhu]),
        ),
    ]
}

/// Columns containing the data to be matched against XOR Bitwise stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn data_for_bitwise<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.cpu.xor) }

/// Column for a binary filter for bitwise instruction in Bitwise stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn filter_for_bitwise<F: Field>() -> Column<F> {
    Column::many(MAP.cpu.inst.ops.ops_that_use_xor())
}

impl<T: Copy> OpSelectors<T> {
    #[must_use]
    pub fn ops_that_use_xor(&self) -> [T; 5] {
        // TODO: Add SRA, once we implement its constraints.
        [self.xor, self.or, self.and, self.srl, self.sll]
    }

    // TODO: Add SRA, once we implement its constraints.
    pub fn ops_that_shift(&self) -> [T; 2] { [self.sll, self.srl] }
}

/// Columns containing the data to be matched against `Bitshift` stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn data_for_shift_amount<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.cpu.bitshift) }

/// Column for a binary filter for shft instruction in `Bitshift` stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn filter_for_shift_amount<F: Field>() -> Column<F> {
    Column::many(MAP.cpu.inst.ops.ops_that_shift())
}

/// Columns containing the data of original instructions.
#[must_use]
pub fn data_for_inst<F: Field>() -> Vec<Column<F>> {
    let inst = MAP.cpu.inst;
    vec![
        Column::single(inst.pc),
        Column::ascending_sum(inst.ops),
        Column::ascending_sum(inst.rs1_select),
        Column::ascending_sum(inst.rs2_select),
        Column::ascending_sum(inst.rd_select),
        Column::single(inst.imm_value),
    ]
}

/// Columns containing the data of permuted instructions.
#[must_use]
pub fn data_for_permuted_inst<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.permuted.inst) }
