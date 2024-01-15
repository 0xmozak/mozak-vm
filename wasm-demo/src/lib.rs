use mozak_runner::instruction::{Args, Instruction, Op};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub fn wasm_demo(a: u32, b: u32) {
    let e = mozak_runner::util::execute_code(
        [Instruction::new(Op::ADD, Args {
            rd: 3,
            rs1: 1,
            rs2: 2,
            ..Args::default()
        })],
        &[],
        &[(1, a), (2, b)],
    );
    let res = mozak_runner::util::state_before_final(&e.1).get_register_value(3);
    alert(&format!("ADD {} {}, RES {}", a, b, res));
}
