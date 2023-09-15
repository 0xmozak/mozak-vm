use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;

use crate::bitshift::columns::Bitshift;
use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cross_table_lookup::Column;
use crate::program::columns::ProgramRom;
use crate::stark::mozak_stark::{CpuTable, Table};
use crate::xor::columns::XorView;

columns_view_impl!(OpSelectors);
/// Selectors for which instruction is currently active.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct OpSelectors<T> {
    pub add: T,
    pub sub: T,
    pub xor: T,
    pub or: T,
    pub and: T,
    pub divu: T,
    /// Remainder Unsigned
    pub remu: T,
    pub mul: T,
    pub mulh: T,
    pub mulhsu: T,
    pub mulhu: T,
    /// Shift Left Logical by amount
    pub sll: T,
    /// Set Less Than
    pub slt: T,
    /// Set Less Than Unsigned comparison
    pub sltu: T,
    /// Shift Right Logical by amount
    pub srl: T,
    /// Jump And Link Register
    pub jalr: T,
    /// Branch on Equal
    pub beq: T,
    /// Branch on Not Equal
    pub bne: T,
    /// Store Byte
    pub sb: T,
    /// Load Byte Unsigned and places it in the least significant byte position
    /// of the target register.
    pub lbu: T,
    /// Branch Less Than
    pub blt: T,
    /// Branch Less Than Unsigned comparison
    pub bltu: T,
    /// Branch Greater or Equal
    pub bge: T,
    /// Branch Greater or Equal Unsigned comparison
    pub bgeu: T,
    /// Environment Call
    pub ecall: T,
}

columns_view_impl!(Instruction);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Instruction<T> {
    /// The original instruction (+ imm_value) used for program
    /// cross-table-lookup.
    pub pc: T,

    /// Selects the current operation type
    pub ops: OpSelectors<T>,
    /// Selects the register to use as source for `rs1`
    pub rs1_select: [T; 32],
    /// Selects the register to use as source for `rs2`
    pub rs2_select: [T; 32],
    /// Selects the register to use as destination for `rd`
    pub rd_select: [T; 32],
    /// Special immediate value used for code constants
    pub imm_value: T,
}

columns_view_impl!(CpuState);
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct CpuState<T> {
    pub clk: T,
    pub inst: Instruction<T>,

    // Represents the end of the program. Also used as the filter column for cross checking Program
    // ROM instructions.
    pub is_running: T,

    pub op1_value: T,
    /// The sum of the value of the second operand register and the
    /// immediate value. Wrapped around to fit in a `u32`.
    pub op2_value: T,
    /// The sum of the value of the second operand and the immediate value as
    /// field elements. Ie summed without wrapping to fit into u32.
    pub op2_value_overflowing: T,
    pub dst_value: T,

    /// Values of the registers.
    pub regs: [T; 32],

    // 0 means non-negative, 1 means negative.
    // (If number is unsigned, it is non-negative.)
    pub op1_sign_bit: T,
    pub op2_sign_bit: T,

    /// `|op1 - op2|`
    pub abs_diff: T,
    /// `1/|op1 - op2| `
    /// It exists only if `op1 != op2`, otherwise assigned to 0.
    pub cmp_diff_inv: T,
    /// If `op1` < `op2`
    pub less_than: T,
    /// normalised_diff == 0 iff op1 == op2
    /// normalised_diff == 1 iff op1 != op2
    /// We need this intermediate variable to keep the constraint degree <= 3.
    pub normalised_diff: T,

    /// Linked values with the Xor Stark Table
    pub xor: XorView<T>,

    /// Linked values with the Bitshift Stark Table
    pub bitshift: Bitshift<T>,

    // Division evaluation columns
    pub quotient: T,
    pub remainder: T,
    /// Value of `divisor - remainder - 1`
    /// Used as a helper column to check that `remainder < divisor`.
    pub remainder_slack: T,
    /// Used as a helper column to check if `divisor` is zero
    pub divisor_inv: T,
    pub divisor: T,

    // Product evaluation columns
    pub op1_abs: T,
    pub op2_abs: T,
    pub skip_check_product_sign: T,
    pub product_sign: T,
    pub product_high_limb: T, // range check u32 required
    pub product_low_limb: T,  // range check u32 required
    /// Used as a helper column to check that `product_high_limb != u32::MAX`
    /// when product_sign is 0 and `product_high_limb != 0` when
    /// product_sign is 1
    pub product_high_limb_inv_helper: T,
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

    // TODO(Matthias): unify where we specify `is_op(1|2)_signed` for constraints
    // and trace generation.
    pub fn is_op2_signed(&self) -> T {
        self.inst.ops.slt + self.inst.ops.bge + self.inst.ops.blt + self.inst.ops.mulh
    }

    pub fn is_op1_signed(&self) -> T { self.is_op2_signed() + self.inst.ops.mulhsu }

    /// Value of the first operand, as if converted to i64.
    ///
    /// For unsigned operations: `Field::from_noncanonical_i64(op1 as i64)`
    /// For signed operations: `Field::from_noncanonical_i64(op1 as i32 as i64)`
    ///
    /// So range is `i32::MIN..=u32::MAX` in Prime Field.
    pub fn op1_full_range(&self) -> T { self.op1_value - self.op1_sign_bit * Self::shifted(32) }

    /// Value of the second operand, as if converted to i64.
    ///
    /// So range is `i32::MIN..=u32::MAX` in Prime Field.
    pub fn op2_full_range(&self) -> T { self.op2_value - self.op2_sign_bit * Self::shifted(32) }

    /// Difference between first and second operands, which works for both pairs
    /// of signed or pairs of unsigned values.
    pub fn signed_diff(&self) -> T { self.op1_full_range() - self.op2_full_range() }
}

/// Expressions we need to range check
///
/// Currently, we only support expressions over the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn rangecheck_looking<F: Field>() -> Vec<Table<F>> {
    let cpu = MAP.cpu.map(Column::from);
    let ops = &cpu.inst.ops;
    let divs = &ops.divu + &ops.remu + &ops.srl;
    let muls = &ops.mul + &ops.mulhu + &ops.mulhsu + &ops.mulh + &ops.sll;

    let is_running = cpu.is_running;

    let is_op2_signed = &ops.slt + &ops.bge + &ops.blt + &ops.mulh;
    let is_op1_signed = &is_op2_signed + &ops.mulhsu;

    vec![
        CpuTable::new(vec![cpu.quotient], divs.clone()),
        CpuTable::new(vec![cpu.remainder], divs.clone()),
        CpuTable::new(vec![cpu.remainder_slack], divs),
        CpuTable::new(vec![cpu.dst_value], &ops.add + &ops.sub + &ops.jalr),
        CpuTable::new(vec![cpu.inst.pc], ops.jalr.clone()),
        CpuTable::new(vec![cpu.abs_diff], &ops.bge + &ops.blt),
        CpuTable::new(vec![cpu.product_high_limb], muls.clone()),
        CpuTable::new(vec![cpu.product_low_limb], muls),
        // apply range constraints for the sign bits of each operand
        CpuTable::new(
            vec![
                cpu.op1_value.clone() - &cpu.op1_sign_bit * F::from_canonical_u64((1_u64) << 32)
                    + &is_op1_signed * F::from_canonical_u64((1_u64) << 31),
            ],
            is_running.clone(),
        ),
        CpuTable::new(
            vec![
                cpu.op2_value.clone() - &cpu.op2_sign_bit * F::from_canonical_u64((1_u64) << 32)
                    + &is_op2_signed * F::from_canonical_u64((1_u64) << 31),
            ],
            is_running,
        ),
    ]
}

/// Columns containing the data to be matched against Xor stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn data_for_xor<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.cpu.xor) }

/// Column for a binary filter for bitwise instruction in Xor stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn filter_for_xor<F: Field>() -> Column<F> {
    MAP.cpu.map(Column::from).inst.ops.ops_that_use_xor()
}

/// Column containing the data to be matched against Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn data_for_memory<F: Field>() -> Vec<Column<F>> { vec![Column::single(MAP.cpu.dst_value)] }

/// Column for a binary filter for memory instruction in Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn filter_for_memory<F: Field>() -> Column<F> { MAP.cpu.map(Column::from).inst.ops.mem_ops() }

impl<T: core::ops::Add<Output = T>> OpSelectors<T> {
    #[must_use]
    pub fn ops_that_use_xor(self) -> T {
        // TODO: Add SRA, once we implement its constraints.
        self.xor + self.or + self.and + self.srl + self.sll
    }

    // TODO: Add SRA, once we implement its constraints.
    pub fn ops_that_shift(self) -> T { self.sll + self.srl }

    // TODO: Add other mem ops like SH, SW, LB, LW, LH, LHU as we implement the
    // constraints.
    pub fn mem_ops(self) -> T { self.sb + self.lbu }
}

/// Columns containing the data to be matched against `Bitshift` stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn data_for_shift_amount<F: Field>() -> Vec<Column<F>> { Column::singles(MAP.cpu.bitshift) }

/// Column for a binary filter for shft instruction in `Bitshift` stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn filter_for_shift_amount<F: Field>() -> Column<F> {
    MAP.cpu.map(Column::from).inst.ops.ops_that_shift()
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
