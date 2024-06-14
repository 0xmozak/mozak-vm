```console
$ target/debug/mozak-cli prove-and-verify -vvv examples/token/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/token-mozakvm --system-tape examples/token/native/out/tape.json --self-prog-id MZK-63236d3b0bc73b9cb18ab2aacbbcf741b84d0560e00172374ddfcffea7b409cc
```

```
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Read 23212 of ELF data.
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Read 2848 of system tape data.
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Self Prog ID: MZK-63236d3b0bc73b9cb18ab2aacbbcf741b84d0560e00172374ddfcffea7b409cc
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Found events: 0
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Length-Prefixed PRIVATE_TAPE    of byte len:     0, on-mem bytes:     4
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Length-Prefixed PUBLIC_TAPE     of byte len:     0, on-mem bytes:     4
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Length-Prefixed CALL_TAPE       of byte len:   504, on-mem bytes:   508
[2024-06-11T10:15:13Z DEBUG mozak_cli::runner] Length-Prefixed EVENT_TAPE      of byte len:     8, on-mem bytes:    12
thread 'main' panicked at /home/matthias/mozak/prog/mozak-vm-7/runner/src/vm.rs:300:13:
Looped for longer than MOZAK_MAX_LOOPS
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

```console
$ target/debug/mozak-cli self-prog-id examples/token/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/token-mozakvm
MZK-9df0168328ecc6dbbd7301064bf11371d039f29eea72fc50f71c1d01c0bf4ad6
```

```console
target/debug/mozak-cli prove-and-verify -vvv examples/token/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/token-mozakvm --system-tape examples/token/native/out/tape.json --self-prog-id MZK-9df0168328ecc6dbbd7301064bf11371d039f29eea72fc50f71c1d01c0bf4ad6
```

After merge with Kapil's branch, I get:

```git-commit
commit c358d70002a389406f597ac98eb9ab6a75d282fe (HEAD -> matthias/bing/rkyv-access-still-fails, origin/matthias/bing/rkyv-access-still-fails)
Merge: a83fe777f a4bbca1ff
Author: Matthias Goergens <matthias.goergens@gmail.com>
Date:   Tue Jun 11 18:18:59 2024 +0800

    Merge remote-tracking branch 'origin/kapil/self_prog_id_dump' into matthias/bing/rkyv-access-still-fails
```

```console
$ target/debug/mozak-cli prove-and-verify -vvv examples/token/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/token-mozakvm --system-tape examples/token/native/out/tape.json
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Read 23212 of ELF data.
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Read 2848 of system tape data.
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Self Prog ID: MZK-9df0168328ecc6dbbd7301064bf11371d039f29eea72fc50f71c1d01c0bf4ad6
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Found events: 2
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Length-Prefixed PRIVATE_TAPE    of byte len:     0, on-mem bytes:     4
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Length-Prefixed PUBLIC_TAPE     of byte len:     0, on-mem bytes:     4
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Length-Prefixed CALL_TAPE       of byte len:   504, on-mem bytes:   508
[2024-06-11T10:20:53Z DEBUG mozak_cli::runner] Length-Prefixed EVENT_TAPE      of byte len:   104, on-mem bytes:   108
thread 'main' panicked at /home/matthias/mozak/prog/mozak-vm-7/runner/src/vm.rs:300:13:
Looped for longer than MOZAK_MAX_LOOPS
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

```
cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv examples/token/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/token-mozakvm --system-tape examples/token/native/out/tape.json 
```
