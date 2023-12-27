# Trace Generation

As mentioned in the [construction of STARK] section, the execution of a program needs to be transformed into AIR. In Mozak RISC-V VM, this means that the compiled RISC-V program in the form of an ELF needs to be decoded and executed. This will then be used to produce an execution trace of the format that can be understood by Starky.

Here are a list of functions that are related to each operation above:

- Decoding: [load_elf]
- Execution: [step]
- Trace Generation: [generate_traces]


[construction of STARK]: starky.md#construction
[load_elf]: https://github.com/0xmozak/mozak-vm/blob/main/runner/src/elf.rs#L136-L194
[step]: https://github.com/0xmozak/mozak-vm/blob/main/runner/src/vm.rs#L377-L405
[generate_traces]: https://github.com/0xmozak/mozak-vm/blob/main/circuits/src/generation/mod.rs#L73-L136