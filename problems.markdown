```
[circuits/src/generation/cpu.rs:185:5] trace.len() = 1
thread 'cpu::add::tests::prove_add_mozak_example' panicked at circuits/src/stark/verifier.rs:80:29:
Failed to verify stark proof for Cpu: Condition failed: `ctl_zs_first.len() == num_ctl_zs` (20 vs 2)
stack backtrace:
   0: rust_begin_unwind
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/std/src/panicking.rs:652:5
   1: core::panicking::panic_fmt
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/panicking.rs:72:14
   2: mozak_circuits::stark::verifier::verify_proof::{{closure}}
             at ./src/stark/verifier.rs:80:29
   3: core::result::Result<T,E>::unwrap_or_else
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/result.rs:1431:23
   4: mozak_circuits::stark::verifier::verify_proof
             at ./src/stark/verifier.rs:72:9
   5: mozak_circuits::test_utils::prove_and_verify_mozak_stark
             at ./src/test_utils.rs:493:5
   6: <mozak_circuits::stark::mozak_stark::MozakStark<<plonky2::plonk::config::Poseidon2GoldilocksConfig as plonky2::plonk::config::GenericConfig<_>>::F,_> as mozak_circuits::test_utils::ProveAndVerify>::prove_and_verify
             at ./src/test_utils.rs:471:9
   7: mozak_circuits::cpu::add::tests::prove_add
             at ./src/cpu/add.rs:51:9
   8: mozak_circuits::cpu::add::tests::prove_add_mozak_example
             at ./src/cpu/add.rs:59:9
   9: mozak_circuits::cpu::add::tests::prove_add_mozak_example::{{closure}}
             at ./src/cpu/add.rs:55:33
  10: core::ops::function::FnOnce::call_once
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/ops/function.rs:250:5
  11: core::ops::function::FnOnce::call_once
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/ops/function.rs:250:5
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
```

Progress!

```
[circuits/src/generation/cpu.rs:185:5] trace.len() = 1
thread 'cpu::add::tests::prove_add_mozak_example' panicked at circuits/src/stark/verifier.rs:80:29:
Failed to verify stark proof for Cpu: Condition failed: `auxiliary_polys.len() == num_auxiliary` (26 vs 6)
stack backtrace:
   0: rust_begin_unwind
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/std/src/panicking.rs:652:5
   1: core::panicking::panic_fmt
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/panicking.rs:72:14
   2: mozak_circuits::stark::verifier::verify_proof::{{closure}}
             at ./src/stark/verifier.rs:80:29
   3: core::result::Result<T,E>::unwrap_or_else
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/result.rs:1431:23
   4: mozak_circuits::stark::verifier::verify_proof
             at ./src/stark/verifier.rs:72:9
   5: mozak_circuits::test_utils::prove_and_verify_mozak_stark
             at ./src/test_utils.rs:493:5
   6: <mozak_circuits::stark::mozak_stark::MozakStark<<plonky2::plonk::config::Poseidon2GoldilocksConfig as plonky2::plonk::config::GenericConfig<_>>::F,_> as mozak_circuits::test_utils::ProveAndVerify>::prove_and_verify
             at ./src/test_utils.rs:471:9
   7: mozak_circuits::cpu::add::tests::prove_add
             at ./src/cpu/add.rs:51:9
   8: mozak_circuits::cpu::add::tests::prove_add_mozak_example
             at ./src/cpu/add.rs:59:9
   9: mozak_circuits::cpu::add::tests::prove_add_mozak_example::{{closure}}
             at ./src/cpu/add.rs:55:33
  10: core::ops::function::FnOnce::call_once
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/ops/function.rs:250:5
  11: core::ops::function::FnOnce::call_once
             at /rustc/244da22fabd9fa677bbd0ac601a88e5ca6917526/library/core/src/ops/function.rs:250:5
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
```
