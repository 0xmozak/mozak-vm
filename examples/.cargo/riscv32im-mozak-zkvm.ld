/*================================================================================*/
/* Default risc-v mozak-zkVM linker script                                        */
/*================================================================================*/
MEMORY {
    riscv_rom(r!wx):   ORIGIN = 0x00000000, LENGTH = 16M   /* used for riscv-attributes */
    mozak_rom(r!wx):   ORIGIN = 0x01000000, LENGTH = 1008M /* used for mozak-zk-vm */
    ram(rw!x):         ORIGIN = 0x40000000, LENGTH = 2048M /* regular ram */
    rom(rx):           ORIGIN = 0xB0000000, LENGTH = 768M  /* ro-data & code - regular rom */
    mozak_stack(rw!x): ORIGIN = 0xF0000000, LENGTH = 256M  /* mozak stack */
}

/*=================================================================================*/
/*    The OUTPUT_ARCH command specifies the machine architecture where the         */
/*    argument is one of the names used in the BFD library. More                   */
/*    specifically one of the entire in bfd/cpu-mips.c                             */
/*=================================================================================*/
OUTPUT_ARCH( "riscv32" )

/*=================================================================================*/
/*  The ENTRY command specifies the entry point (ie. first instruction to execute) */
/*  The symbol _start is defined in crt0.S                                         */
/*=================================================================================*/
ENTRY( _start )

SECTIONS {
    /* [0] - RISCV rom (NOT IN USE) */
    /*
        .riscv : { *(.riscv); *(.riscv.attributes); *(.riscv.attributes.*); } > riscv_rom
        _riscv_start = ADDR(.riscv);
        _riscv_end = _riscv_start + SIZEOF(.riscv);
    */

    /* [1] - mozak globals */
    _mozak_merkle_state_root          = 0x01000000; /* assumed size is 32 bytes */
    _mozak_merkle_state_root_capacity = 0x00000100; /* 32 bytes */
    _mozak_timestamp                  = 0x01000100; /* assumed size of timestamp is 4 bytes, the rest can be used */
    _mozak_timestamp_capacity         = 0x00000008; /* 8 bytes */
    _mozak_empty_space                = 0x01000108; /* empty space - reserved for future use */
    _mozak_empty_space_capacity       = 0x0EFFFEF8; /* empty space capacity - 0x10000000 - 0x01000108 */
    _mozak_public_io_tape             = 0x10000000; /* assumed size is 256 MB */
    _mozak_public_io_tape_capacity    = 0x10000000; /* 256 MB in bytes */
    _mozak_private_io_tape            = 0x20000000; /* assumed size is 512 MB */
    _mozak_private_io_tape_capacity   = 0x20000000; /* 512 MB in bytes */

    /* [2] - SHSTRTAB rom */
    .shstrtab : { *(.shstrtab .shstrtab.*) } > ram
    _shstrtab_start = ADDR(.shstrtab);
    _shstrtab_end = _shstrtab_start + SIZEOF(.shstrtab);

    /* [3] - STRTAB rom */
    .strtab : { *(.strtab .strtab.*) } > ram
    _strtab_start = ADDR(.strtab);
    _strtab_end = _strtab_start + SIZEOF(.strtab);

    /* [4] - SYMTAB rom */
    .symtab : { *(.symtab .symtab.*) } > ram
    _symtab_start = ADDR(.symtab);
    _symtab_end = _symtab_start + SIZEOF(.symtab);

    /* [5] - BSS ram */
    .bss : { *(.bss); *(.bss.*) } > ram
    _bss_start = ADDR(.bss);
    _bss_load_start = LOADADDR(.bss);
    _bss_end = _bss_start + SIZEOF(.bss);

    /* [6] - SBSS ram */
    .sbss : { *(.sbss); *(.sbss.*); *(.sdata); *(.sdata.*) } > ram
    _sbss_start = ADDR(.sbss);
    _sbss_load_start = LOADADDR(.sbss);
    _sbss_end = _bss_start + SIZEOF(.sbss);

    /* [7] - DATA ram */
    .data : { *(.data); *(.data.*); *(.data1); *(.data1.*) } > ram
    _data_start = ADDR(.data);
    _data_load_start = LOADADDR(.data);
    _data_end = _data_start + SIZEOF(.data);

    /* [8] - End of uninitialized data segment (used for heap) - mozak heap grows up */
    _mozak_heap_start = _data_end;
    .mozak_heap : {} > ram

    /* [9] - TEXT rom */
    .text : { *(.text); *(.text.*); *(.init) *(.init.*); *(.fini); *(.fini.*) } > rom
    _text_start = ADDR(.text);
    _text_end = _text_start + SIZEOF(.text);

    /* [10] - RODATA rom */
    .rodata : { *(.rodata .rodata.*); *(.rodata1 .rodata1.*) } > rom
    _rodata_start = ADDR(.rodata);
    _rodata_end = _rodata_start + SIZEOF(.rodata);

    /* [11] - mozak stack - grows down */
    _mozak_stack_top = 0xFFFFFFFF;
    .mozak_stack : {} > mozak_stack

    /* [12] - Init mozak_stack with 0xFF (NOT IN USE - runtime increased cause of FILL ) */
    /*
        .mozak_stack : {
            . = _mozak_stack_top;
            FILL(0x00)
            . -= 256M;
        } > mozak_stack
    */

    /* [13] - don't insert these sections */
    /DISCARD/ : {
        *(.note .comment);
        *(.riscv .riscv.*);
        *(.debug .debug.*);
        *(.line .line.*);
        *(.relname .relname.* .relaname .relaname.*);
        *(.dynstr .dynstr.* .dynsym .dynsym.* .dynamic .dynamic.*);
        *(.eh_frame .eh_frame.* .glink .glink.* .got .got.* .plt .plt.*);
        *(.toc .toc.* .tocbss .tocbss.*);
        *(.branch_lt .branch_lt.* .opd .opd.*);
        *(.rela.dyn .rela.dyn.* .rela.plt .rela.plt.* .toc1 .toc1.*)
    }
}

ASSERT(_rodata_end < 0xF0000000, "risc-v mozak linker script: program is too large")
