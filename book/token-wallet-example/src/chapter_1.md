# High level overview

Lets setup the scenario.

Alice owns a USDC token in her USDC wallet. She has to transfer the token to Bob, who has his own USDC wallet.

A USDC token is represented as `StateObject` with constraint owner being USDC token program represented through `ProgramIdentifier`. 

```rust
struct StateObject{
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
- - read `usdc_token_object` from global state
  - ensure its data matches
