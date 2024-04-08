# Memory Constraints

Loading and Storing to Memory are also constrained as a Starky trace in Mozak-VM.

Consider the following Memory trace

| Is Executed | ADDR | CLK | OP | VALUE | DIFF_ADDR | DIFF_ADDR_INV       | DIFF_CLK |
| ----------- | ---- | ----| -- | ----- | --------- | ------------------- | -------- |
| 1           | 100  | 0   | SB | 5     | 100       | 3504881373188771021 | 0        |
| 1           | 100  | 1   | LB | 5     | 0         | 0                   | 1        |
| 1           | 100  | 4   | SB | 10    | 0         | 0                   | 3        |
| 1           | 100  | 5   | LB | 10    | 0         | 0                   | 1        |
| 1           | 200  | 2   | SB | 15    | 100       | 3504881373188771021 | 0        |
| 1           | 200  | 3   | LB | 15    | 0         | 0                   | 1        |
| 0           | 200  | 3   | LB | 15    | 0         | 0                   | 0        |
| 0           | 200  | 3   | LB | 15    | 0         | 0                   | 0        |

Here are what the columns stands for
- Is Executed: Whether the instruction is executed. 1 means true. 0 means false.
- ADDR: Address of Memory.
- CLK: A clock counter of that starts at 0 and is increased by 1 at each step of the execution.
- OP: Operand that is either a Store Byte (SB) or a Load Byte (LB). At this stage, all forms of memory
  access, such as Load Full Word and Load Half Word, are converted to a single type of memory access.
- VALUE: The value get or used by the operand.
- DIFF_ADDR: How much the addree of this row is different from the previous row.
- DIFF_ADDR_INV: Inverse of DIFF_ADDR. This is useful for further constraints.
- DIFF_CLK: How much the clock of this row is different from the previous row.

At trace generation phase, We sorted the memory access trace based first on ADDR then on CLK. A Cross Table Lookup argument is used to ensure that the memory trace is the same before and after the reordering.

Like All STARK constraints. We have initialization constraints that constrain the initial values in the cells and transition constraints that constrain
how the values in the cells should be updated.

<!-- moved from docs in the circuits/src/memory/stark.rs -->
## Memory initialization Constraints

The memory table is assumed to be ordered by `addr` in ascending order.
such that whenever we describe an memory init / access
pattern of an "address", a correct table guarantees the following:

All rows for a specific `addr` MUST start with one, or both, of:
  1) a zero init (case for heap / other dynamic addresses).
  2) a memory init via static ELF (hereby referred to as elf init), or
For these starting rows, `is_init` will be true.

1) Zero Init
  All zero initialized memory will have clk `0` and value `0`. They
  should also be writable.

2) ELF Init
  All elf init rows will have clk `1`.

In principle, zero initializations for a certain address MUST come
before any elf initializations to ensure we don't zero out any memory
initialized by the ELF. This is constrained via a rangecheck on `diff_clk`.
Since clk is in ascending order, any memory address with a zero init
(`clk` == 0) after an elf init (`clk` == 1) would be caught by
this range check.

## Memory Transition Constraints

Define `new_addr = DIFF_ADDR * DIFF_ADDR_INV `. This value can either be 1 or 0. If it is 1, we switched to
a new address and vice versa.

1. If `new_addr`, `OP = SB`. If we have a new addrees, the first operand must be a store.
2. If not `new_addr`, `DIFF_CLK (next row) <= CLK (next row) - CLK (this row)`. If we are at the same address, the clock difference
   between rows must be less or equal to the clock difference.
3. If `new_address`, `DIFF_CLK == 0`. `DIFF_CLK` is set to 0 at new address.
4. `DIFF_ADDR (next row) <= ADDR (next row) - ADDR (this row)` The address difference between rows is always less or equal to the difference
    of address between rows.
5. If `OP (next row) == LB`, `VALUE (next row) == VALUE (this row)`. Load should not change values.
6. `(new_addr - 1) * DIFF_ADDR == 0` and `(new_addr - 1) * DIFF_ADDR_INV == 0`. This constrains the relationship among `new_addr`, `DIFF_ADDR` and
   `DIFF_ADDR_INV`
7. `(IS_EXECUTED (this row) - IS_EXECUTED (next row)) * IS_EXECUTED (next row) == 0`. Constraints on the padding rows of the trace.
