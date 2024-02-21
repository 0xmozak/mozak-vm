# High level overview

Lets setup the scenario.

Alice owns a USDC token in her USDC wallet. She has to transfer the token to Bob, who has his own USDC wallet.

A USDC token is represented as `StateObject` with constraint owner being USDC token program represented through `ProgramIdentifier`.

```rust

let struct StateObject{
	/// location in the state Patricia tree
	address: [bool; DEPTH] // TODO: update to exact datatype
	/// The only program who can mutate data field
	constraint_owner: ProgramIdentifier
	/// blob of data
	data: &[u8] 
}

struct ProgramIdenitifer{
	/// commitment to read only data from ELF
	program_rom_hash: Poseidon2Hash
	/// commitment to memory init table
	memory_init_hash: Poseidon2Hash
	/// the instruction at which program execution starts
	entry_point: usize
}
let usdc_token_program = ProgramIdentifier {
        program_rom_hash: [11, 113, 20, 251].into(),
        memory_init_hash: [2, 31, 3, 62].into(),
        entry_point: 0,
 };

let usdc_token_owner = ALICE_PUBLIC_KEY;

let usdc_token_object = StateObject{
	address: [1, 0, 0].into(),
	constraint_owner: usdc_token_program
	data: usdc_token_owner.to_bytes(),

}
```

The USDC token transfer also needs to interact with wallets of Alice and Bob. Namely it needs approval from their respective wallet programs to do the token transfer.

```rust
S    let alice_wallet = ProgramIdentifier {
        program_rom_hash: [21, 90, 121, 87].into(),
        memory_init_hash: [31, 35, 20, 189].into(),
        entry_point: 0,
    };

    let bob_wallet = ProgramIdentifier {
        program_rom_hash: [0, 2, 121, 187].into(),
        memory_init_hash: [180, 19, 19, 56].into(),
        entry_point: 0,
    };
```

on high level, the programs be responsible for the following:

- `usdc_token_program` :
- - "send" a request to `alice_wallet` program to approve the transfer of `usdc_token_object` to `bob_wallet`.
  - once it "receives" an approval from the program, it changes the owner of `usdc_token_program` to Bob, by updating the public key
  - finally, it "broadcasts" that it has changed the object's state.
- `alice_wallet`
- - "receive" request of approval from `alice_wallet` to do the transfer of `usdc_token_object` to `bob_wallet`
  - "broadcast" that it has read the `usdc_token_object`
  - check that the public key mentioned in `usdc_token_object` indeed corresponds to `alice_wallet`'s private key. And `bob_wallet` corresponds to the wallet it indeed wants to transfer the token object to.
  - "send" back the approval to `usdc_token_program`

But what exactly would happen when we say "send", "receive" and "broadcast" occur?
The reality is that these won't occur in usual sense. That is, "send" or "receive" don't correspond to a program calling another program or waiting for a response from other program. Nor "broadcast" refers to sending it some entity who is listening.

What actually would happen is that these programs would demonstrate that they have actually followed a script together, complied with other's requests, as well as sent the intended responses. Each of the program continues the execution as if it had made the "call" with correct arguements,  "received" the intended response, and "broadcasted" the intended state change.
This entire script is stored in two parts. `CallTape` is the part where the "call" and "receive" events are stored. While `EventTape` is the part where all the proposed changes to final state are stored.

Now how these tapes are created? The idea is that the play is performed, and then the script is created. Each program has two types of execution, native and zkvm. In the native execution all the "call", "receive" and "broadcast" are emulated in the intended manner, and the `CallTape` and `EventTape` is generated. In the zkvm execution, the program,  the actual functions mentioned in the `CallTape`  are executed by corresponding program, and their output is shown to be the same as the ones mentioned in the `CallTape`.

In this scenario, the `CallTape` would attest to following events

- `token_program` called `alice_wallet` to execute the `approve_transfer` function with arguments `(alice_wallet, usdc_token_object, ))`
