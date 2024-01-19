#![allow(dead_code, unused_imports)]
use mozak_circuits::test_utils::prove_and_verify_mozak_stark;
use mozak_runner::instruction::{Args, Instruction, Op};
use starky::config::StarkConfig;
use wasm_bindgen::prelude::*;

extern crate console_error_panic_hook;
use std::panic;

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub fn wasm_demo(a: u32, b: u32) {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
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
    let config = StarkConfig::standard_fast_config();

    alert(&format!("ADD {} {}, RES {}", a, b, res));
    let proving_res = prove_and_verify_mozak_stark(&e.0, &e.1, &config);
    alert(&format!("Proving :{}", proving_res.is_ok()));
}


pub fn wasm_demo_(a: u32, b: u32) {
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
    let config = StarkConfig::standard_fast_config();

    println!("ADD {} {}, RES {}", a, b, res);
    let proving_res = prove_and_verify_mozak_stark(&e.0, &e.1, &config);
    println!("Proving :{}", proving_res.is_ok());
}