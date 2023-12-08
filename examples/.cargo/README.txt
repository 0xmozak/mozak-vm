/*====================================================================================================================*/
/* Resources that helped to develop this script                                                                       */
/*====================================================================================================================*/
/*
 * 1)  General info about linker-script, C & Rust statics etc ... (This is the most important info to read)
 *     https://mcyoung.xyz/2021/06/01/linker-script/
 * 2)  About FILL / fill commands
 *     https://mcuoneclipse.com/2014/06/23/filling-unused-memory-with-the-gnu-linker/
 * 3)  Examples of linked-scripts (RICS-V)
 *     https://github.com/Lichtso/riscv-llvm-templates/blob/master/src/spike.lds
 * 4)  RISC-V attributes sections
 *     https://github.com/riscv-non-isa/riscv-elf-psabi-doc/blob/master/riscv-elf.adoc#rv-section
 * 5)  Rust info about linker-scripts
 *     https://docs.rust-embedded.org/embedonomicon/memory-layout.html
 * 6)  LLVM Linker
 *     https://lld.llvm.org/ELF/linker_script.html
 * 7)  Linux Loader
 *     http://www.dbp-consulting.com/tutorials/debugging/linuxProgramStartup.html
 * 8)  More about init/fini
 *     https://maskray.me/blog/2021-11-07-init-ctors-init-array
 * 9)  Linux exec sys-call
 *     https://linuxhint.com/linux-exec-system-call/
 * 10) About Rust run-time
 *     https://ductile.systems/rusts-runtime/
 * 11) More code about Rust run-time
 *     https://github.com/rust-lang/rust/blob/master/library/std/src/rt.rs
 * 12) Loader / ELF by Ulrich Drepper
 *     https://akkadia.org/drepper/dsohowto.pdf
 * 13) Linux kernel task-struct
 *     https://docs.huihoo.com/doxygen/linux/kernel/3.7/structtask__struct.html
 * 14) Another linker-script guide
 *     https://www.phaedsys.com/principals/emprog/emprogdata/thunderbench-Linker-Script-guide.pdf
 */

/* Useful commands:
 * 1) Show sections: `riscv64-unknown-elf-readelf -S ELF_EXE`
 * 2) Disassembly: `riscv64-unknown-elf-objdump -D ELF_EXE | riscv64-unknown-elf-c++filt`
 * 3) Show all symbols: `riscv64-unknown-elf-nm -a ELF_EXE | riscv64-unknown-elf-c++filt`