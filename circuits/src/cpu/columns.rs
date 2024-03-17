use plonky2::field::extension::Extendable;
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use crate::bitshift::columns::Bitshift;
use crate::columns_view::{columns_view_impl, make_col_map, NumberOfColumns};
use crate::cpu::stark::add_extension_vec;
use crate::cross_table_lookup::{Column, ColumnX};
use crate::memory::columns::MemoryCtl;
use crate::memory_io::columns::InputOutputMemoryCtl;
use crate::poseidon2_sponge::columns::Poseidon2SpongeCtl;
use crate::program::columns::{InstructionRow, ProgramRom};
use crate::rangecheck::columns::RangeCheckCtl;
use crate::stark::mozak_stark::{BitshiftTable, CpuTable, MemoryTable, TableNamed, XorTable};
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
    pub div: T,
    pub rem: T,
    pub mul: T,
    // MUL High: Multiply two values, and return most significant 'overflow' bits
    pub mulh: T,
    /// Shift Left Logical by amount
    pub sll: T,
    /// Set Less Than
    pub slt: T,
    /// Shift Right Logical by amount
    pub srl: T,
    /// Arithmetic Right Shifts
    pub sra: T,
    /// Jump And Link Register
    pub jalr: T,
    /// Branch on Equal
    pub beq: T,
    /// Branch on Not Equal
    pub bne: T,
    /// Store Byte
    pub sb: T,
    /// Store Half Word
    pub sh: T,
    /// Store Word
    pub sw: T,
    /// Load Byte Unsigned and places it in the least significant byte position
    /// of the target register.
    pub lb: T,
    /// Load Half Word
    pub lh: T,
    /// Load Word
    pub lw: T,
    /// Branch Less Than
    pub blt: T,
    /// Branch Greater or Equal
    pub bge: T,
    /// Environment Call
    pub ecall: T,
}

columns_view_impl!(Instruction);
/// Internal [Instruction] of Stark used for transition constrains
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct Instruction<T> {
    /// The original instruction (+ imm_value) used for program
    /// cross-table-lookup.
    pub pc: T,

    /// Selects the current operation type
    pub ops: OpSelectors<T>,
    pub is_op1_signed: T,
    pub is_op2_signed: T,
    pub is_dst_signed: T,
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
/// Represents the State of the CPU, which is also a row of the trace
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

    /// `dst_value` contains "correct" (modified from `mem_access_raw` for
    /// signed operations) value targetted towards `dst`.
    pub dst_value: T,
    pub dst_sign_bit: T,

    /// `mem_access_raw` contains values fetched or stored into the memory
    /// table. These values are always unsigned by nature (as mem table does
    /// not differentiate between signed and unsigned values).
    pub mem_value_raw: T,

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
    pub op2_value_inv: T,
    pub quotient_value: T, // range check u32 required
    pub quotient_sign: T,
    pub skip_check_quotient_sign: T,
    pub remainder_value: T, // range check u32 required
    pub remainder_sign: T,
    /// Value of `divisor_abs - remainder_abs - 1`
    /// Used as a helper column to check that `remainder < divisor`.
    pub remainder_slack: T, // range check u32 required

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
    pub mem_addr: T,
    pub io_addr: T,
    pub io_size: T,
    pub is_io_store_private: T,
    pub is_io_store_public: T,
    pub is_io_transcript: T,
    pub is_halt: T,
    pub is_poseidon2: T,
    // TODO: these two need constraints.
    // (And/or should probably be removed.)
    pub poseidon2_input_addr: T,
    pub poseidon2_input_len: T,
}

make_col_map!(CpuColumnsExtended);
pub(crate) const CPU_MAP: CpuState<CpuCol> = COL_MAP.cpu;

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

    /// The value of the designated register in rs2.
    pub fn rs2_value(&self) -> T {
        // Note: we could skip 0, because r0 is always 0.
        // But we keep it to make it easier to reason about the code.
        (0..32)
            .map(|reg| self.inst.rs2_select[reg] * self.regs[reg])
            .sum()
    }

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

pub fn rs2_value_extension_target<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    cpu: &CpuState<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    let mut rs2_value = builder.zero_extension();
    for reg in 0..32 {
        let rs2_select = builder.mul_extension(cpu.inst.rs2_select[reg], cpu.regs[reg]);
        rs2_value = builder.add_extension(rs2_value, rs2_select);
    }
    rs2_value
}

pub fn op1_full_range_extension_target<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    cpu: &CpuState<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    let shifted_32 = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let op1_sign_bit = builder.mul_extension(cpu.op1_sign_bit, shifted_32);
    builder.sub_extension(cpu.op1_value, op1_sign_bit)
}

pub fn op2_full_range_extension_target<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    cpu: &CpuState<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    let shifted_32 = builder.constant_extension(F::Extension::from_canonical_u64(1 << 32));
    let op2_sign_bit = builder.mul_extension(cpu.op2_sign_bit, shifted_32);
    builder.sub_extension(cpu.op2_value, op2_sign_bit)
}

pub fn signed_diff_extension_target<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    cpu: &CpuState<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    let op1_full_range = op1_full_range_extension_target(builder, cpu);
    let op2_full_range = op2_full_range_extension_target(builder, cpu);
    builder.sub_extension(op1_full_range, op2_full_range)
}

/// Expressions we need to range check
///
/// Currently, we only support expressions over the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn rangecheck_looking() -> Vec<TableNamed<RangeCheckCtl<Column>>> {
    let cpu = CPU_MAP;
    let ops = cpu.inst.ops;
    let divs = ops.div + ops.rem + ops.srl + ops.sra;
    let muls = ops.mul + ops.mulh + ops.sll;

    let new: fn(CpuCol) -> RangeCheckCtl<CpuCol> = RangeCheckCtl::new;
    vec![
        CpuTable::new(new(cpu.quotient_value), divs),
        CpuTable::new(new(cpu.remainder_value), divs),
        CpuTable::new(new(cpu.remainder_slack), divs),
        CpuTable::new(new(cpu.dst_value), ops.add + ops.sub + ops.jalr),
        CpuTable::new(new(cpu.inst.pc), ops.jalr),
        CpuTable::new(new(cpu.abs_diff), ops.bge + ops.blt),
        CpuTable::new(new(cpu.product_high_limb), muls),
        CpuTable::new(new(cpu.product_low_limb), muls),
        // apply range constraints for the sign bits of each operand
        CpuTable::new(
            new(cpu.op1_value - cpu.op1_sign_bit * (1 << 32) + cpu.inst.is_op1_signed * (1 << 31)),
            cpu.inst.is_op1_signed,
        ),
        CpuTable::new(
            new(cpu.op2_value - cpu.op2_sign_bit * (1 << 32) + cpu.inst.is_op2_signed * (1 << 31)),
            cpu.inst.is_op2_signed,
        ),
        CpuTable::new(
            new(cpu.dst_value - cpu.dst_sign_bit * 0xFFFF_FF00),
            cpu.inst.ops.lb,
        ),
        CpuTable::new(
            new(cpu.dst_value - cpu.dst_sign_bit * 0xFFFF_0000),
            cpu.inst.ops.lh,
        ),
    ]
}

/// Lookup into Xor stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_xor() -> TableNamed<XorView<Column>> {
    XorTable::new(CPU_MAP.xor, CPU_MAP.inst.ops.ops_that_use_xor())
}

/// Column containing the data to be matched against Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_memory() -> TableNamed<MemoryCtl<Column>> {
    let cpu = CPU_MAP;
    MemoryTable::new(
        MemoryCtl {
            clk: cpu.clk,
            is_store: cpu.inst.ops.sb,
            is_load: cpu.inst.ops.lb, // For both `LB` and `LBU`
            addr: cpu.mem_addr,
            value: cpu.mem_value_raw,
        },
        CPU_MAP.inst.ops.byte_mem_ops(),
    )
}

type CpuCol = ColumnX<CpuColumnsExtended<i64>>;

/// Lookup into half word Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_halfword_memory() -> TableNamed<MemoryCtl<Column>> {
    let cpu = CPU_MAP;
    CpuTable::new(
        MemoryCtl {
            clk: cpu.clk,
            is_store: cpu.inst.ops.sh,
            is_load: cpu.inst.ops.lh,
            addr: cpu.mem_addr,
            value: cpu.mem_value_raw,
        },
        CPU_MAP.inst.ops.halfword_mem_ops(),
    )
}

/// Lookup into Fullword Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_fullword_memory() -> TableNamed<MemoryCtl<Column>> {
    let cpu = CPU_MAP;
    MemoryTable::new(
        MemoryCtl {
            clk: cpu.clk,
            is_store: cpu.inst.ops.sw,
            is_load: cpu.inst.ops.lw,
            addr: cpu.mem_addr,
            value: cpu.mem_value_raw,
        },
        cpu.inst.ops.fullword_mem_ops(),
    )
}

#[allow(clippy::large_types_passed_by_value)]
fn lookup_for_io_memory_x(
    filter: ColumnX<CpuColumnsExtended<i64>>,
) -> TableNamed<InputOutputMemoryCtl<Column>> {
    let cpu = CPU_MAP;
    CpuTable::new(
        InputOutputMemoryCtl {
            clk: cpu.clk,
            addr: cpu.io_addr,
            size: cpu.io_size,
        },
        filter,
    )
}

/// Column containing the data to be matched against IO Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_io_memory_private() -> TableNamed<InputOutputMemoryCtl<Column>> {
    lookup_for_io_memory_x(CPU_MAP.is_io_store_private)
}

// TODO: consolidate lookup_for_io_memory_private and
// lookup_for_io_memory_public and data_for_io_transcript into a single lookup
// to save implicit CPU lookups columns.
#[must_use]
pub fn lookup_for_io_memory_public() -> TableNamed<InputOutputMemoryCtl<Column>> {
    lookup_for_io_memory_x(CPU_MAP.is_io_store_public)
}

#[must_use]
pub fn lookup_for_io_transcript() -> TableNamed<InputOutputMemoryCtl<Column>> {
    lookup_for_io_memory_x(CPU_MAP.is_io_transcript)
}

impl<T: core::ops::Add<Output = T>> OpSelectors<T> {
    #[must_use]
    pub fn ops_that_use_xor(self) -> T {
        self.xor + self.or + self.and + self.srl + self.sll + self.sra
    }

    pub fn ops_that_shift(self) -> T { self.sll + self.srl + self.sra }

    pub fn byte_mem_ops(self) -> T { self.sb + self.lb }

    pub fn halfword_mem_ops(self) -> T { self.sh + self.lh }

    pub fn fullword_mem_ops(self) -> T { self.sw + self.lw }

    pub fn is_mem_ops(self) -> T { self.sb + self.lb + self.sh + self.lh + self.sw + self.lw }
}

pub fn is_mem_op_extention_target<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    ops: &OpSelectors<ExtensionTarget<D>>,
) -> ExtensionTarget<D> {
    add_extension_vec(builder, vec![
        ops.sb, ops.lb, ops.sh, ops.lh, ops.sw, ops.lw,
    ])
}

/// Lookup into `Bitshift` stark.
#[must_use]
pub fn lookup_for_shift_amount() -> TableNamed<Bitshift<Column>> {
    BitshiftTable::new(CPU_MAP.bitshift, CPU_MAP.inst.ops.ops_that_shift())
}

/// Columns containing the data of original instructions.
#[must_use]
pub fn lookup_for_inst() -> TableNamed<InstructionRow<Column>> {
    let inst = CPU_MAP.inst;
    CpuTable::new(
        InstructionRow {
            pc: inst.pc,
            // Combine columns into a single column.
            // - ops: This is an internal opcode, not the opcode from RISC-V, and can fit within 5
            //   bits.
            // - is_op1_signed and is_op2_signed: These fields occupy 1 bit each.
            // - rs1_select, rs2_select, and rd_select: These fields require 5 bits each.
            // - imm_value: This field requires 32 bits.
            // Therefore, the total bit requirement is 5 * 6 + 32 = 62 bits, which is less than the
            // size of the Goldilocks field.
            // Note: The imm_value field, having more than 5 bits, must be positioned as the last
            // column in the list to ensure the correct functioning of 'reduce_with_powers'.
            inst_data: ColumnX::reduce_with_powers(
                [
                    ColumnX::ascending_sum(inst.ops),
                    inst.is_op1_signed,
                    inst.is_op2_signed,
                    ColumnX::ascending_sum(inst.rs1_select),
                    ColumnX::ascending_sum(inst.rs2_select),
                    ColumnX::ascending_sum(inst.rd_select),
                    inst.imm_value,
                ],
                1 << 5,
            ),
        },
        CPU_MAP.is_running,
    )
}

/// Lookup of permuted instructions.
#[must_use]
pub fn lookup_for_permuted_inst() -> TableNamed<InstructionRow<Column>> {
    CpuTable::new(COL_MAP.permuted.inst, COL_MAP.cpu.is_running)
}

/// Lookup of permuted instructions.
#[must_use]
pub fn lookup_for_program_rom() -> TableNamed<InstructionRow<Column>> {
    CpuTable::new(COL_MAP.permuted.inst, COL_MAP.permuted.filter)
}

#[must_use]
pub fn lookup_for_poseidon2_sponge() -> TableNamed<Poseidon2SpongeCtl<Column>> {
    let cpu: CpuState<ColumnX<CpuColumnsExtended<i64>>> = CPU_MAP;
    CpuTable::new(
        Poseidon2SpongeCtl {
            clk: cpu.clk,
            input_addr: cpu.poseidon2_input_addr,
            input_len: cpu.poseidon2_input_len,
        },
        CPU_MAP.is_poseidon2,
    )
}
