include!(concat!(env!("OUT_DIR"), "/vars.rs"));

/// This macro takes in an identifier as the test name and the file name of a
/// compiled ELF, and sets up a `run_test` for it.
macro_rules! load_test_elf {
    ($file_name:tt) => {
        include_bytes!(concat!("../../riscv-testdata/testdata/", $file_name))
    };
}

// TODO: fix the macro.
#[allow(non_upper_case_globals)]
pub static riscv_tests: &[&[u8]] = &[
    // Base instruction set
    load_test_elf!("rv32ui-p-add"),
    load_test_elf!("rv32ui-p-addi"),
    load_test_elf!("rv32ui-p-and"),
    load_test_elf!("rv32ui-p-andi"),
    load_test_elf!("rv32ui-p-auipc"),
    load_test_elf!("rv32ui-p-beq"),
    load_test_elf!("rv32ui-p-bge"),
    load_test_elf!("rv32ui-p-bgeu"),
    load_test_elf!("rv32ui-p-blt"),
    load_test_elf!("rv32ui-p-bltu"),
    load_test_elf!("rv32ui-p-bne"),
    load_test_elf!("rv32ui-p-jal"),
    load_test_elf!("rv32ui-p-jalr"),
    load_test_elf!("rv32ui-p-lb"),
    load_test_elf!("rv32ui-p-lbu"),
    load_test_elf!("rv32ui-p-lh"),
    load_test_elf!("rv32ui-p-lhu"),
    load_test_elf!("rv32ui-p-lui"),
    load_test_elf!("rv32ui-p-lw"),
    load_test_elf!("rv32ui-p-or"),
    load_test_elf!("rv32ui-p-ori"),
    load_test_elf!("rv32ui-p-sb"),
    load_test_elf!("rv32ui-p-sh"),
    load_test_elf!("rv32ui-p-simple"),
    load_test_elf!("rv32ui-p-sll"),
    load_test_elf!("rv32ui-p-slli"),
    load_test_elf!("rv32ui-p-slt"),
    load_test_elf!("rv32ui-p-slti"),
    load_test_elf!("rv32ui-p-sltiu"),
    load_test_elf!("rv32ui-p-sltu"),
    load_test_elf!("rv32ui-p-sra"),
    load_test_elf!("rv32ui-p-srai"),
    load_test_elf!("rv32ui-p-srl"),
    load_test_elf!("rv32ui-p-srli"),
    load_test_elf!("rv32ui-p-sub"),
    load_test_elf!("rv32ui-p-sw"),
    load_test_elf!("rv32ui-p-xor"),
    load_test_elf!("rv32ui-p-xori"),
    // M extension
    load_test_elf!("rv32um-p-div"),
    load_test_elf!("rv32um-p-divu"),
    load_test_elf!("rv32um-p-mul"),
    load_test_elf!("rv32um-p-mulh"),
    load_test_elf!("rv32um-p-mulhsu"),
    load_test_elf!("rv32um-p-mulhu"),
    load_test_elf!("rv32um-p-rem"),
    load_test_elf!("rv32um-p-remu"),
];
