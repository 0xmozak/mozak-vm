use mozak_vm::elf::Program;
use mozak_vm::state::State;
use mozak_vm::vm::Vm;

macro_rules! test_elf {
    ($elf_filename:ident) => {
        #[test]
        fn $elf_filename() {
            let _ = env_logger::try_init();
            let elf = std::fs::read(format!(
                "tests/testdata/rv32ui-p-{}",
                stringify!($elf_filename)
            ))
            .unwrap();
            let max_mem_size = 1024 * 1024 * 1024; // 1 GB
            let program = Program::load_elf(&elf, max_mem_size);
            assert!(program.is_ok());
            let program = program.unwrap();
            let state = State::new(program);
            let mut vm = Vm::new(state);
            let res = vm.step();
            assert!(res.is_ok());
        }
    };
}

test_elf!(addi);
test_elf!(and);
test_elf!(andi);
test_elf!(auipc);
test_elf!(beq);
test_elf!(bge);
test_elf!(bgeu);
test_elf!(blt);
test_elf!(bltu);
test_elf!(bne);
// test_elf!(fence_i);
// test_elf!(div);
// test_elf!(divu);
test_elf!(jal);
test_elf!(jalr);
test_elf!(lb);
test_elf!(lbu);
test_elf!(lh);
test_elf!(lhu);
test_elf!(lui);
test_elf!(lw);
// test_elf!(mul);
// test_elf!(mulh);
// test_elf!(mulhsu);
// test_elf!(mulhu);
test_elf!(or);
test_elf!(ori);
// test_elf!(rem);
// test_elf!(remu);
test_elf!(sb);
test_elf!(sh);
test_elf!(simple);
test_elf!(sll);
test_elf!(slli);
test_elf!(slt);
test_elf!(slti);
test_elf!(sltiu);
test_elf!(sltu);
test_elf!(sra);
test_elf!(srai);
test_elf!(srl);
test_elf!(srli);
test_elf!(sub);
test_elf!(sw);
test_elf!(xor);
test_elf!(xori);
