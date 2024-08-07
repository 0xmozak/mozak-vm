# Trace Generation

As mentioned in the [construction of STARK] section, the execution of a program needs to be transformed into AIR. In Mozak RISC-V VM, this means that the compiled RISC-V program in the form of an ELF needs to be decoded and executed. This will then be used to produce an execution trace of the format that can be understood by Starky.

Here are a list of functions that are related to each operation above:

- Decoding: [load_elf]
- Execution: [step]
- Trace Generation: [generate_traces]

## Decoding
Programs are compiled down to `risc32im-mozak-mozakvm-elf`. This is a custom RISC-V target specified by this [json] file. If you are interested in what these flags in the json file standard for, check out the target options spec [here].

## Execution
In the execution phase, we simulate the RISC-V processor state with registers and memories. Each decoded instruction update the state and all of them produce a RISC-V state trace.

## Trace Generation
This is the final step before constraining and proving the execution. Traces are converted into Starky rows. We also add lookup argument traces and custom gate traces such as poseidon2 traces at this step.


[construction of STARK]: starky.md#construction
[load_elf]: https://github.com/0xmozak/mozak-vm/blob/main/runner/src/elf.rs#L136-L194
[step]: https://github.com/0xmozak/mozak-vm/blob/main/runner/src/vm.rs#L377-L405
[generate_traces]: https://github.com/0xmozak/mozak-vm/blob/main/circuits/src/generation/mod.rs#L73-L136
[json]: https://github.com/0xmozak/mozak-vm/blob/main/examples/.cargo/riscv32im-mozak-mozakvm-elf.json
[here]: https://docs.rust-embedded.org/embedonomicon/custom-target.html#fill-the-target-file
