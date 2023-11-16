# VM sub-crate

This sub-crate contains the logic necessary to:
- Load the RISC-V _(RISC32I+M)_ program from the ELF file representation
- Run the RISC-V program and provide its execution trace. It implements all the operations of [
  _RISC32I+M_](https://github.com/jameslzhu/riscv-card/blob/master/riscv-card.pdf) specification.

The purpose of the sub-crate is to emulate the VM and provide its execution trace, which can be then used inside ZK
prover to prove code execution.
