use core::ops::{Add, Mul, Sub};

use crate::bitshift::columns::Bitshift;
use crate::columns_view::{columns_view_impl, make_col_map};
use crate::cross_table_lookup::{Column, ColumnWithTypedInput};
use crate::memory::columns::MemoryCtl;
use crate::poseidon2_sponge::columns::Poseidon2SpongeCtl;
use crate::program::columns::ProgramRom;
use crate::rangecheck::columns::RangeCheckCtl;
use crate::register::RegisterCtl;
use crate::stark::mozak_stark::{CpuTable, TableWithTypedOutput};
use crate::storage_device::columns::StorageDeviceCtl;
use crate::xor::columns::XorView;

columns_view_impl!(OpSelectors);
/// Selectors for which instruction is currently active.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
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
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Instruction<T> {
    /// The original instruction (+ `imm_value`) used for program
    /// cross-table-lookup.
    pub pc: T,

    /// Selects the current operation type
    pub ops: OpSelectors<T>,
    pub is_op1_signed: T,
    pub is_op2_signed: T,
    pub is_dst_signed: T,
    /// Selects the register to use as source for `rs1`
    pub rs1_selected: T,
    /// Selects the register to use as source for `rs2`
    pub rs2_selected: T,
    /// Selects the register to use as destination for `rd`
    pub rd_selected: T,
    /// Special immediate value used for code constants
    pub imm_value: T,
}

make_col_map!(CpuState);
columns_view_impl!(CpuState);
/// Represents the State of the CPU, which is also a row of the trace
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct CpuState<T> {
    pub clk: T,
    pub inst: Instruction<T>,

    // Represents the end of the program. Also used as the filter column for cross checking Program
    // ROM instructions.
    pub is_running: T,

    pub op1_value: T,
    pub op2_value_raw: T,
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
    /// `normalised_diff` == 0 iff op1 == op2
    /// `normalised_diff` == 1 iff op1 != op2
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
    /// when `product_sign` is 0 and `product_high_limb != 0` when
    /// `product_sign` is 1
    pub product_high_limb_inv_helper: T,
    pub mem_addr: T,
    pub storage_device_addr: T,
    pub storage_device_size: T,
    // We don't need all of these 'is_<some-ecall>' columns.  Because our CPU table (by itself)
    // doesn't need to be deterministic. We can assert these things in the CTL-ed
    // ecall-specific tables.
    // But to make that work, all ecalls need to be looked up; so we can use ops.ecall as the
    // filter.
    // TODO: implement the above.
    pub is_private_tape: T,
    pub is_public_tape: T,
    pub is_call_tape: T,
    pub is_event_tape: T,
    pub is_events_commitment_tape: T,
    pub is_cast_list_commitment_tape: T,
    pub is_halt: T,
    pub is_poseidon2: T,
}
pub(crate) const CPU: &CpuState<ColumnWithTypedInput<CpuState<i64>>> = &COL_MAP;

impl<T> CpuState<T>
where
    T: Copy + Add<Output = T> + Mul<i64, Output = T> + Sub<Output = T>,
{
    /// Value of the first operand, as if converted to i64.
    ///
    /// For unsigned operations: `Field::from_noncanonical_i64(op1 as i64)`
    /// For signed operations: `Field::from_noncanonical_i64(op1 as i32 as i64)`
    ///
    /// So range is `i32::MIN..=u32::MAX` in Prime Field.
    pub fn op1_full_range(&self) -> T { self.op1_value - self.op1_sign_bit * (1 << 32) }

    /// Value of the second operand, as if converted to i64.
    ///
    /// So range is `i32::MIN..=u32::MAX` in Prime Field.
    pub fn op2_full_range(&self) -> T { self.op2_value - self.op2_sign_bit * (1 << 32) }

    /// Difference between first and second operands, which works for both pairs
    /// of signed or pairs of unsigned values.
    pub fn signed_diff(&self) -> T { self.op1_full_range() - self.op2_full_range() }
}

impl<P: Copy + Add<Output = P>> OpSelectors<P>
where
    i64: Sub<P, Output = P>,
{
    // List of opcodes that manipulated the program counter, instead of
    // straight line incrementing it.
    // Note: ecall is only 'jumping' in the sense that a 'halt'
    // does not bump the PC. It sort-of jumps back to itself.
    pub fn is_jumping(&self) -> P {
        self.beq + self.bge + self.blt + self.bne + self.ecall + self.jalr
    }

    /// List of opcodes that only bump the program counter.
    pub fn is_straightline(&self) -> P { 1 - self.is_jumping() }

    /// List of opcodes that work with memory.
    pub fn is_mem_op(&self) -> P { self.sb + self.lb + self.sh + self.lh + self.sw + self.lw }
}

/// Expressions we need to range check
///
/// Currently, we only support expressions over the
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn rangecheck_looking() -> Vec<TableWithTypedOutput<RangeCheckCtl<Column>>> {
    let ops = &CPU.inst.ops;
    let divs = ops.div + ops.rem + ops.srl + ops.sra;
    let muls: ColumnWithTypedInput<CpuState<i64>> = ops.mul + ops.mulh + ops.sll;

    [
        (CPU.quotient_value, divs),
        (CPU.remainder_value, divs),
        (CPU.remainder_slack, divs),
        (CPU.dst_value, ops.add + ops.sub + ops.jalr),
        (CPU.inst.pc, ops.jalr),
        (CPU.abs_diff, ops.bge + ops.blt),
        (CPU.product_high_limb, muls),
        (CPU.product_low_limb, muls),
        // apply range constraints for the sign bits of each operand
        // TODO(Matthias): these are a bit suspicious, because the filter also appears in the data.
        // Carefully review!
        (
            CPU.op1_value - CPU.op1_sign_bit * (1 << 32) + CPU.inst.is_op1_signed * (1 << 31),
            CPU.inst.is_op1_signed,
        ),
        (
            CPU.op2_value - CPU.op2_sign_bit * (1 << 32) + CPU.inst.is_op2_signed * (1 << 31),
            CPU.inst.is_op2_signed,
        ),
        (CPU.dst_value - CPU.dst_sign_bit * 0xFFFF_FF00, ops.lb),
        (CPU.dst_value - CPU.dst_sign_bit * 0xFFFF_0000, ops.lh),
    ]
    .into_iter()
    .map(|(columns, filter)| CpuTable::new(RangeCheckCtl(columns), filter))
    .collect()
}

/// Lookup for Xor stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_xor() -> TableWithTypedOutput<XorView<Column>> {
    CpuTable::new(CPU.xor, CPU.inst.ops.ops_that_use_xor())
}

/// Lookup into Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_memory() -> TableWithTypedOutput<MemoryCtl<Column>> {
    CpuTable::new(
        MemoryCtl {
            clk: CPU.clk,
            is_store: CPU.inst.ops.sb,
            is_load: CPU.inst.ops.lb, // For both `LB` and `LBU`
            addr: CPU.mem_addr,
            value: CPU.mem_value_raw,
        },
        CPU.inst.ops.byte_mem_ops(),
    )
}

/// Lookup into half word Memory stark.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_halfword_memory() -> TableWithTypedOutput<MemoryCtl<Column>> {
    CpuTable::new(
        MemoryCtl {
            clk: CPU.clk,
            is_store: CPU.inst.ops.sh,
            is_load: CPU.inst.ops.lh,
            addr: CPU.mem_addr,
            value: CPU.mem_value_raw,
        },
        CPU.inst.ops.halfword_mem_ops(),
    )
}

/// Lookup into fullword Memory table.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_fullword_memory() -> TableWithTypedOutput<MemoryCtl<Column>> {
    CpuTable::new(
        MemoryCtl {
            clk: CPU.clk,
            is_store: CPU.inst.ops.sw,
            is_load: CPU.inst.ops.lw,
            addr: CPU.mem_addr,
            value: CPU.mem_value_raw,
        },
        CPU.inst.ops.fullword_mem_ops(),
    )
}

/// Column containing the data to be matched against StorageDevice starks.
/// [`CpuTable`](crate::cross_table_lookup::CpuTable).
#[must_use]
pub fn lookup_for_storage_tables() -> TableWithTypedOutput<StorageDeviceCtl<Column>> {
    CpuTable::new(
        StorageDeviceCtl {
            op: ColumnWithTypedInput::ascending_sum([
                CPU.is_private_tape,
                CPU.is_public_tape,
                CPU.is_call_tape,
                CPU.is_event_tape,
                CPU.is_events_commitment_tape,
                CPU.is_cast_list_commitment_tape,
            ]),
            clk: CPU.clk,
            addr: CPU.storage_device_addr,
            size: CPU.storage_device_size,
        },
        [
            CPU.is_private_tape,
            CPU.is_public_tape,
            CPU.is_call_tape,
            CPU.is_event_tape,
            CPU.is_events_commitment_tape,
            CPU.is_cast_list_commitment_tape,
        ]
        .iter()
        .sum(),
    )
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
}

/// Lookup into `Bitshift` stark.
#[must_use]
pub fn lookup_for_shift_amount() -> TableWithTypedOutput<Bitshift<Column>> {
    CpuTable::new(CPU.bitshift, CPU.inst.ops.ops_that_shift())
}

/// Columns containing the data of original instructions.
#[must_use]
pub fn lookup_for_program_rom() -> TableWithTypedOutput<ProgramRom<Column>> {
    let inst = CPU.inst;
    CpuTable::new(
        ProgramRom {
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
            inst_data: ColumnWithTypedInput::reduce_with_powers(
                [
                    ColumnWithTypedInput::ascending_sum(inst.ops),
                    inst.is_op1_signed,
                    inst.is_op2_signed,
                    inst.rs1_selected,
                    inst.rs2_selected,
                    inst.rd_selected,
                    inst.imm_value,
                ],
                1 << 5,
            ),
        },
        ColumnWithTypedInput::constant(1),
    )
}

#[must_use]
pub fn lookup_for_poseidon2_sponge() -> TableWithTypedOutput<Poseidon2SpongeCtl<Column>> {
    CpuTable::new(Poseidon2SpongeCtl { clk: CPU.clk }, CPU.is_poseidon2)
}

#[must_use]
pub fn register_looking() -> Vec<TableWithTypedOutput<RegisterCtl<Column>>> {
    let is_read = ColumnWithTypedInput::constant(1);
    let is_write = ColumnWithTypedInput::constant(2);

    vec![
        CpuTable::new(
            RegisterCtl {
                clk: CPU.clk,
                op: is_read,
                addr: CPU.inst.rs1_selected,
                value: CPU.op1_value,
            },
            CPU.is_running,
        ),
        CpuTable::new(
            RegisterCtl {
                clk: CPU.clk,
                op: is_read,
                addr: CPU.inst.rs2_selected,
                value: CPU.op2_value_raw,
            },
            CPU.is_running,
        ),
        CpuTable::new(
            RegisterCtl {
                clk: CPU.clk,
                op: is_write,
                addr: CPU.inst.rd_selected,
                value: CPU.dst_value,
            },
            CPU.is_running,
        ),
    ]
}
