~/m/p/mozak-vm (matthias/rkyv-access)$ cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv examples/target/riscv32im-mozak-mozakvm-elf/release/walletbin --system-tape examples/wallet/out/tape.json --self-prog-id MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.05s
     Running `target/debug/mozak-cli prove-and-verify -vvv examples/target/riscv32im-mozak-mozakvm-elf/release/walletbin --system-tape examples/wallet/out/tape.json --self-prog-id MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb`
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Read 32668 of ELF data.
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Read 916 of system tape data.
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Self Prog ID: MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Found events: 0cp 
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Length-Prefixed PRIVATE_TAPE    of byte len:    32, on-mem bytes:    36
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Length-Prefixed PUBLIC_TAPE     of byte len:     0, on-mem bytes:     4
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Length-Prefixed CALL_TAPE       of byte len:   236, on-mem bytes:   240
[2024-05-31T05:12:38Z DEBUG mozak_cli::runner] Length-Prefixed EVENT_TAPE      of byte len:     8, on-mem bytes:    12
thread 'main' panicked at /home/matthias/mozak/prog/mozak-vm/runner/src/vm.rs:300:13:
Looped for longer than MOZAK_MAX_LOOPS
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


+ cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv examples/target/riscv32im-mozak-mozakvm-elf/release/walletbin --system-tape examples/wallet/out/tape.json --self-prog-id MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.06s
     Running `target/debug/mozak-cli prove-and-verify -vvv examples/target/riscv32im-mozak-mozakvm-elf/release/walletbin --system-tape examples/wallet/out/tape.json --self-prog-id MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb`
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Read 25580 of ELF data.
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Read 916 of system tape data.
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Self Prog ID: MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Found events: 0
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Length-Prefixed PRIVATE_TAPE    of byte len:    32, on-mem bytes:    36
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Length-Prefixed PUBLIC_TAPE     of byte len:     0, on-mem bytes:     4
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Length-Prefixed CALL_TAPE       of byte len:   236, on-mem bytes:   240
[2024-05-31T05:13:48Z DEBUG mozak_cli::runner] Length-Prefixed EVENT_TAPE      of byte len:     8, on-mem bytes:    12
[2024-05-31T05:13:48Z DEBUG mozak_circuits::stark::prover] Starting Prove
[2024-05-31T05:13:48Z DEBUG mozak_circuits::generation] Starting Trace Generation
[2024-05-31T05:13:48Z DEBUG mozak_circuits::cpu::generation] Starting CPU Trace Generation
[2024-05-31T05:13:48Z DEBUG mozak_circuits::stark::prover] Done with Trace Generation
[2024-05-31T05:13:48Z DEBUG plonky2::util::timing] TimingTree is not supported without the 'timing' feature enabled
[2024-05-31T05:13:48Z DEBUG mozak_circuits::stark::verifier] Starting Verify

~/m/p/mozak-vm (matthias/rkyv-access)$ cp examples/target/riscv32im-mozak-mozakvm-elf/release/walletbin long-walletbin
~/m/p/mozak-vm (matthias/rkyv-access)$ file long-walletbin 
long-walletbin: ELF 32-bit LSB executable, UCB RISC-V, soft-float ABI, version 1 (SYSV), statically linked, not stripped
