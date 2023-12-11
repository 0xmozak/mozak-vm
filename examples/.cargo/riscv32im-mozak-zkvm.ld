/*====================================================================================================================*/
/* Default risc-v mozak-zkVM linker script                                                                            */
/*====================================================================================================================*/

/*====================================================================================================================*/
/* This linker-script was tested on LLD LLVM linker, it may or may not work with another linkers, the syntax and the  */
/* parameters are sometime different from linker to linker. This linker script mostly oriented to release-build,      */
/* since it throw away debug sections. Currently debug build works as expected without it, but maybe we will have two */
/* scripts, one for release and one for debug, later                                                                  */
/* Useful info howto add/remove something from this script:                                                           */
/* 1) Add / Remove / Modify                                                                                           */
/* 2) Enable linker map-file generation (refer to config.toml)                                                        */
/* 3) Check that map layout looks as we expect                                                                        */
/* 4) Use nm, objdump to list the sections, symbols, disassembly - don't assume, check !                              */
/* 5) All this is very experimental and not documented, low-level, toolchain and specific version dependent (can be)  */
/* 6) Look for useful links in README.ld file                                                                         */
/*====================================================================================================================*/

/*====================================================================================================================*/
/* Mozak-zkVM memory layout                                                                                           */
/* - riscv_rom defined since compiler insert symbols related to it, currently these symbols are discarded, but it     */
/*   it maybe used in future. Another reason: looks like somehow it is good practice to leave some region started     */
/*   specifically on address ZERO.                                                                                    */
/* - RAM -> ROM -> STACK schema is used because it will help to catch out-of-bounds writes, since ROM is read-only    */
/*====================================================================================================================*/
MEMORY {
    riscv_rom(r!wx):   ORIGIN = 0x00000000, LENGTH = 16M   /* used for riscv-attributes */
    mozak_rom(r!wx):   ORIGIN = 0x01000000, LENGTH = 1008M /* used for mozak-zk-vm */
    ram(rw!x):         ORIGIN = 0x40000000, LENGTH = 2048M /* regular ram */
    rom(rx):           ORIGIN = 0xB0000000, LENGTH = 768M  /* ro-data & code - regular rom */
    mozak_stack(rw!x): ORIGIN = 0xF0000000, LENGTH = 256M  /* mozak stack */
}

/*====================================================================================================================*/
/*  The OUTPUT_ARCH command specifies the machine architecture where the                                              */
/*  argument is one of the names used in the BFD library. More                                                        */
/*  specifically one of the entire in bfd/cpu-mips.c                                                                  */
/*====================================================================================================================*/
OUTPUT_ARCH( "riscv32" )

/*====================================================================================================================*/
/*  The ENTRY command specifies the entry point (ie. first instruction to execute)                                    */
/*  The symbol _start is defined in crt0.S                                                                            */
/*====================================================================================================================*/
ENTRY( _start )

/*====================================================================================================================*/
/*  The _start & _end symbols are defined since currently we don't have mechanism to extract sections by name         */
/*  so, this is the only why to fully parse mozak-style-elf by mozak-elf-loader                                       */
/*  later on, when and if this mechanism will be introduced and tested, these symbols can be removed                  */
/*  but, having these defined is very convenient for linker-scripts and it does not affect proving time and not       */
/*  final memory layout                                                                                               */
/*====================================================================================================================*/
SECTIONS {
    /* [0] - RISCV rom (NOT IN USE), since riscv attributes section is discarded */
    /*       It still here, since maybe in future we will want this section back */
    /*       Since linker-scripts are very undocumented and each linker slightly */
    /*       differs, and because actually these symbols indeed produced by      */
    /*       compiles, it was decided to leave this section here - commented-out */
    /*
        .riscv : { *(.riscv); *(.riscv.attributes); *(.riscv.attributes.*); } > riscv_rom
        _riscv_start = ADDR(.riscv);
        _riscv_end = _riscv_start + SIZEOF(.riscv);
    */

    /* [1] - mozak globals */
    .mozak_globals : {
        _mozak_merkle_state_root          = 0x01000000; /* assumed size is 32 bytes */
        _mozak_merkle_state_root_capacity = 0x00000100; /* 32 bytes */
        _mozak_empty_space                = 0x01000100; /* empty space - reserved for future use */
        _mozak_empty_space_capacity       = 0x0EFFFF00; /* empty space capacity - 0x10000000 - 0x01000100 */
        _mozak_public_io_tape             = 0x10000000; /* assumed size is 256 MB */
        _mozak_public_io_tape_capacity    = 0x10000000; /* 256 MB in bytes */
        _mozak_private_io_tape            = 0x20000000; /* assumed size is 512 MB */
        _mozak_private_io_tape_capacity   = 0x20000000; /* 512 MB in bytes */
    }

    /* [2] - SHSTRTAB - no region - address = 0, can't be placed in region, only place in sections table */
    .shstrtab : { *(.shstrtab .shstrtab.*) }
    _shstrtab_start = ADDR(.shstrtab);
    _shstrtab_end = _shstrtab_start + SIZEOF(.shstrtab);

    /* [3] - STRTAB - no region - address = 0, can't be placed in region, only place in sections table */
    .strtab : { *(.strtab .strtab.*) }
    _strtab_start = ADDR(.strtab);
    _strtab_end = _strtab_start + SIZEOF(.strtab);

    /* [4] - SYMTAB - no region - address = 0, can't be placed in region, only place in sections table */
    .symtab : { *(.symtab .symtab.*) }
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
    .mozak_heap : {
        _mozak_heap_start = _data_end;
        _mozak_heap_end = 0xB0000000;
    } > ram

    /* [9] - TEXT rom */
    .text : { *(.text); *(.text.*); *(.init) *(.init.*); *(.fini); *(.fini.*) } > rom
    _text_start = ADDR(.text);
    _text_end = _text_start + SIZEOF(.text);

    /* [10] - RODATA rom */
    .rodata : { *(.rodata .rodata.*); *(.rodata1 .rodata1.*) } > rom
    _rodata_start = ADDR(.rodata);
    _rodata_end = _rodata_start + SIZEOF(.rodata);

    /* [11] - mozak stack - grows down */
    .mozak_stack : {
        _mozak_stack_end = 0xF0000000;
        _mozak_stack_top = 0xFFFFFFFF;
    } > mozak_stack

    /* [12] - Init mozak_stack with 0xFF (NOT IN USE - runtime increased cause of FILL ) */
    /*        This section + FILL works as expected but increase instruction count etc ..*/
    /*        Prefer to leave it here, maybe in use later                                */
    /*
        .mozak_stack : {
            . = _mozak_stack_top;
            FILL(0x00)
            . -= 256M;
        } > mozak_stack
    */

    /* [13] - don't insert these sections */
    /DISCARD/ : {
        /******************************************************/
        /* [13.1] - sections related to note & comment        */
        /******************************************************/
        *(.note .comment);

        /******************************************************/
        /* [13.2] - riscv sections - maybe will be used later */
        /******************************************************/
        *(.riscv .riscv.*);

        /******************************************************/
        /* [13.3] - debug sections                            */
        /******************************************************/
        *(.debug .debug.* .debug_*);
        *(.line .line.*);

        /******************************************************/
        /* [13.4] - exceptions - not supported at all         */
        /******************************************************/
        *(.eh_frame .eh_frame.*);

        /******************************************************/
        /* [13.5] - dynamic symbols, only static is supported */
        /******************************************************/
        *(.glink .glink.* .got .got.* .plt .plt.*);
        *(.relname .relname.* .relaname .relaname.*);
        *(.dynstr .dynstr.* .dynsym .dynsym.* .dynamic .dynamic.*);
        *(.rela.dyn .rela.dyn.* .rela.plt .rela.plt.*);
        *(.toc .toc.* .tocbss .tocbss.* .toc1 .toc1.*);
        *(.branch_lt .branch_lt.* .opd .opd.*);

        /******************************************************/
        /* [13.6] - all other stuff that is not supported     */
        /******************************************************/
        *(.interp .interp.*)
    }
}

/* 0xF000_0000 is a bottom of mozak-stack, so ro_data-end needs to be less */
ASSERT(_rodata_end < 0xF0000000, "risc-v mozak linker script: program is too large")
