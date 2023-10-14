# Architecture

Mozak-VM is a [STARK] based [RISC-V] virtual machine where a Prover $\mathcal{P}$ can generate a proof $\pi$ for the execution of a RISC-V program. A verifier $\mathcal{V}$ can easily verify the correct execution of the program through $\pi$ without re-executing the program.


### `runner`

The `runner` crate emulates the RISC-V virtual machine. It execute an [ELF] program and provide the program's execution trace.

`runner/src/instruction.rs`: List of RISC-V instructions implemented

`runner/src/elf.rs`: Responsible for parsing an ELF Program

`runner/src/decode.rs`: Responsible for decoding each instruction

`runner/src/system.rs`: RISC-V constants

`runner/src/state.rs`: Virtual Machine state. eg. registers, memory.

`runner/src/vm.rs`: Responsiber for execution of each instruction

### `circuits`

see [README.md](/circuits/README.md) of circuits






[STARK]: https://eprint.iacr.org/2018/046
[RISC-V]: https://riscv.org/technical/specifications/
[ELF]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format