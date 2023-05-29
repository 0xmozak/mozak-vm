use anyhow::Result;
use mozak_vm::elf::Program;
use mozak_vm::state::State;
use mozak_vm::vm::Vm;

macro_rules! test_elf {
    ($test_name:ident, $file_name:tt) => {
        #[test]
        fn $test_name() -> Result<()> {
            let _ = env_logger::try_init();
            let elf_name = format!("tests/testdata/{}", $file_name);
            let elf = std::fs::read(elf_name)?;
            let program = Program::load_elf(&elf)?;
            let state = State::from(program);
            let mut vm = Vm::new(state);
            vm.step().map(|_| ())?;
            // At the end of every test,
            // register a0(x10) is set to 0 before an ECALL if it passes
            assert_eq!(vm.state.get_register_value(10_usize), 0);
            assert_eq!(vm.state.get_register_value(17_usize), 93);
            assert!(vm.state.has_halted());

            Ok(())
        }
    };
}

// Base instruction set
test_elf!(add, "rv32ui-p-add");
test_elf!(addi, "rv32ui-p-addi");
test_elf!(and, "rv32ui-p-and");
test_elf!(andi, "rv32ui-p-andi");
test_elf!(auipc, "rv32ui-p-auipc");
test_elf!(beq, "rv32ui-p-beq");
test_elf!(bge, "rv32ui-p-bge");
test_elf!(bgeu, "rv32ui-p-bgeu");
test_elf!(blt, "rv32ui-p-blt");
test_elf!(bltu, "rv32ui-p-bltu");
test_elf!(bne, "rv32ui-p-bne");
test_elf!(jal, "rv32ui-p-jal");
test_elf!(jalr, "rv32ui-p-jalr");
test_elf!(lb, "rv32ui-p-lb");
test_elf!(lbu, "rv32ui-p-lbu");
test_elf!(lh, "rv32ui-p-lh");
test_elf!(lhu, "rv32ui-p-lhu");
test_elf!(lui, "rv32ui-p-lui");
test_elf!(lw, "rv32ui-p-lw");
test_elf!(or, "rv32ui-p-or");
test_elf!(ori, "rv32ui-p-ori");
test_elf!(sb, "rv32ui-p-sb");
test_elf!(sh, "rv32ui-p-sh");
test_elf!(simple, "rv32ui-p-simple");
test_elf!(sll, "rv32ui-p-sll");
test_elf!(slli, "rv32ui-p-slli");
test_elf!(slt, "rv32ui-p-slt");
test_elf!(slti, "rv32ui-p-slti");
test_elf!(sltiu, "rv32ui-p-sltiu");
test_elf!(sltu, "rv32ui-p-sltu");
test_elf!(sra, "rv32ui-p-sra");
test_elf!(srai, "rv32ui-p-srai");
test_elf!(srl, "rv32ui-p-srl");
test_elf!(srli, "rv32ui-p-srli");
test_elf!(sub, "rv32ui-p-sub");
test_elf!(sw, "rv32ui-p-sw");
test_elf!(xor, "rv32ui-p-xor");
test_elf!(xori, "rv32ui-p-xori");

// Unimplemented
// test_elf!(fence_i, "rv32ui-");

// M extension
test_elf!(div, "rv32um-p-div");
test_elf!(divu, "rv32um-p-divu");
test_elf!(mul, "rv32um-p-mul");
test_elf!(mulh, "rv32um-p-mulh");
test_elf!(mulhsu, "rv32um-p-mulhsu");
test_elf!(mulhu, "rv32um-p-mulhu");
test_elf!(rem, "rv32um-p-rem");
test_elf!(remu, "rv32um-p-remu");
