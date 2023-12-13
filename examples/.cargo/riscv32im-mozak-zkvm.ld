/*====================================================================================================================*/
/* Default risc-v mozak-zkVM linker script                                                                            */
/* ---------------------------------------                                                                            */
/* This linker-script was tested on LLD LLVM linker, it may or may not work with another linkers, the syntax and the  */
/* parameters are sometime different from linker to linker.                                                           */
/*====================================================================================================================*/

/* Defines the target architecture to build for with an entry symbol, here `_start`                                   */
OUTPUT_ARCH( "riscv32" )
ENTRY( _start )

/*====================================================================================================================*/
/* Mozak-zkVM memory layout                                                                                           */
/*====================================================================================================================*/
MEMORY {
    /* We reserve the first few bytes for any "future" use. This inludes for sections like .risc_attributes */
    /* Space used: 0x00000000 to 0x00FFFFFF                                                                 */
    riscv_rom(r!wx):   ORIGIN = 0x00000000, LENGTH = 16M

    /* Executable code section, right after reserved, we give it 240M so that `riscv_rom` + `readonly_code` */
    /* combine together to take up a good whole number 256M.                                                */
    /* Space used: 0x01000000 to 0x0FFFFFFF                                                                 */
    readonly_code(rx): ORIGIN = 0x01000000, LENGTH = 240M
    
    /* Static data section, to be populated with "globals" from program ELF                                 */
    /* Space used: 0x10000000 to 0x1FFFFFFF                                                                 */
    elf_globals(rw):   ORIGIN = 0x10000000, LENGTH = 256M

    /* Static data section to be populated with blockchain "globals" or context-specific mozak variables    */
    /* Space used: 0x20000000 to 0x20FFFFFF                                                                 */
    mozak_rom(r):      ORIGIN = 0x20000000, LENGTH = 16M

    /* BSS entries need some space, we similarly allocate 240M for it so that `mozak_rom` and `bss` together */
    /* use up a good whole number 256M.                                                                      */
    /* Space used: 0x21000000 to 0x2FFFFFFF                                                                  */
    bss(rw):           ORIGIN = 0x21000000, LENGTH = 240M

    /* General purpose RAM is worth 3GB for us                                                               */
    /* Space used: 0x30000000 to 0xEFFFFFFF                                                                  */
    ram(rw!x):         ORIGIN = 0x30000000, LENGTH = 3G

    /* General purpose STACK for program to use                                                              */
    /* Space used: 0xF0000000 to 0xFFFFFFFF                                                                  */
    mozak_stack(rw!x): ORIGIN = 0xF0000000, LENGTH = 256M
}
