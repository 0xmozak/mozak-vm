/*
====================================================================================================================  
 Default risc-v mozak-zkVM linker script                                                                            
 ---------------------------------------                                                                            
 This linker-script was tested on LLD LLVM linker, it may or may not work with another linkers, the syntax and the  
 parameters are sometime different from linker to linker.                                                           
====================================================================================================================
*/

/* 
Defines the target architecture to build for with an entry symbol, here `_start`
*/
OUTPUT_ARCH( "riscv32" )
ENTRY( _start )

/*
Mozak-zkVM memory layout, loosely based on general idea as in `docs/memory_layout.md`
*/
MEMORY {
    /* We reserve the first few bytes for any "future" use. This inludes for sections like .risc_attributes */
    riscv_rom(r!wx):    ORIGIN = 0x00000000, LENGTH = 16M

    /* 
    Executable code section, right after reserved, we give it 240M so that `riscv_rom` + `readonly_code`
    combine together to take up a good whole number 256M.                                                
    */
    readonly_code(rx):  ORIGIN = 0x01000000, LENGTH = 240M
    
    /* Static data section, to be populated with rodata from program ELF */
    elf_rodata(r!x):    ORIGIN = 0x10000000, LENGTH = 256M

    /* Static data section to be populated with blockchain "globals" or context-specific mozak variables */
    mozak_context(r!x): ORIGIN = 0x20000000, LENGTH = 768M

    /* Uninitialised and initialized memory from ELF */
    elf_rwdata(rw!x):   ORIGIN = 0x50000000, LENGTH = 256M

    /* General purpose RAM is worth 2.3GB for us */
    ram(rw!x):          ORIGIN = 0x60000000, LENGTH = 2304M

    /* 
    General purpose STACK for program to use, leaving last few bytes for "kernel", left over for 
    consistency with the established layout, but that we don't really need it. The following is commented
    out since these values are not used right now.
    TODO: find if we can affect this, if we want to affect this.
    */
    /* mozak_stack(rw!x):  ORIGIN = 0xF0000000, LENGTH = 240M */
}

_heap_start = ORIGIN(ram);

SECTIONS {
    /* For memory region: `riscv_rom` --------------------------------------------------------------------   */

    /* 
    Used to contain `.risc_attributes`. However, it may be  dropped since we know exact target
    attributes. This can also be directly dropped during compilation via `-mno-arch-attr` flag.
    We ensure using the `DISCARD` section below that this is removed.
    */

    /* For memory region: `readonly_code` ----------------------------------------------------------------   */
    .text : { *(.text); *(.text.*); *(.init) *(.init.*); *(.fini); *(.fini.*) } > readonly_code

    /* For memory region: `elf_rodata` -------------------------------------------------------------------   */
    .rodata : { *(.rodata .rodata.*); *(.rodata1 .rodata1.*) } > elf_rodata

    /* For memory region: `mozak_context` ----------------------------------------------------------------   */

    /* mozak_globals, NOTE: needs to sync with memory regions defined above */
    .mozak_globals : {
        _mozak_self_prog_id               = 0x20000000;
        _mozak_cast_list                  = 0x20000020;
        _mozak_public_tape             = 0x21000000;
        _mozak_private_tape            = 0x30000000;
        _mozak_call_tape                  = 0x40000000;
        _mozak_event_tape                 = 0x48000000;
    } > mozak_context

    /* For memory region: `elf_rwdata` -------------------------------------------------------------------   */

    /* BSS uninitialised memory */
    .bss : { *(.bss); *(.bss.*) } > elf_rwdata

    /* SBSS uninitialised memory */
    .sbss : { *(.sbss); *(.sbss.*); *(.sdata); *(.sdata.*) } > elf_rwdata

    /* Data initialised memory */
    .data : { *(.data); *(.data.*); *(.data1); *(.data1.*) } > elf_rwdata

    /* For memory region: `ram` ------------------------------ EMPTY -------------------------------------   */

    /* DISCARD to forcefully remove sections */
    /DISCARD/ : {
        /* 
        Removes `.risc_attributes` and family. Refer memory region `riscv_rom` mem section for
        explanation
        */
        *(.riscv .riscv.*);

        /* Removes debugs, comments, unnecessary for execution */
        *(.debug .debug.* .debug_*);
        *(.line .line.*);
        *(.note .comment);
    }
}
