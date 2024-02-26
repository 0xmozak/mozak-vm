# High level overview

Consider the following scenario.

Alice owns a USDC token in her  wallet. She has to transfer the token to Bob, who has his own wallet.

A USDC token is represented as `StateObject` with the constraint owner being the USDC token program represented through `ProgramIdentifier`. The USDC token, along with the amount, also stores the details of its owner (in this case, Alice) and her wallet program. The USDC program would require approval from Alice's wallet program in order to transfer the economic ownership to Bob and his wallet program.

```rust
let alice_wallet: ProgramIdentifier;
let bob_wallet: ProgramIdentifier;

let usdc_token_object_data = USDCTokenData{
	usdc_token_owner: ALICE_PUBLIC_KEY;
	wallet_program: alice_wallet;
	amount: 100;
}

let usdc_token_program: ProgramIdentifier;

let usdc_token_object = StateObject{
	address: address_of_object_in_state_tree,
	constraint_owner: usdc_token_program
	data: usdc_token_object_data.to_bytes(),
}
```

At a high level, It would look like the program did the following:

- `usdc_token_program` :
- - request `alice_wallet` to "make a call"  to function that approves the transfer of `usdc_token_object` to `bob_wallet`.
- - gets "output of call", that is, approval of the transfer
  - "propose a change" of `usdc_token_object`'s owner and wallet to that of Bob
- `alice_wallet`
- - receives, from `usdc_token_program`  "request to call"  the function to approve transfer of `usdc_token_object `to `bob_wallet`
  - the function execution shows ownership of `usdc_token_object` by proving knowledge of private key corresponding to public key of `usdc_token_object`, which is supplied by Alice to the program through private tape
  - sends "output of call", that is the approval of transfer back to `usdc_token_program`

But what exactly would happen when we say "make a call", get or send "output of call" and "propose a change" occur?
In practice, these won't occur in the usual sense. "Make a call" or send "output of call" don't correspond to a program calling another program or sending a response to other program. "Propose a change" does not refer to sending it to some entity who is listening in.

What would actually happen is that these programs would demonstrate that they have actually followed a script together, complied with each other's requests, and sent and received the intended responses. Each of the programs continues the execution as if it had "made the call" with correct arguments, computed the correct "output of call", and sent the intended response and "proposed" the intended state change.
This entire script is stored in two parts. `CallTape` is the part where the "make a call" and "output of call" events are stored. `EventTape` is the part where all the proposed changes to final state are stored.

We briefly describe how these tapes are created. The idea is that the play is performed, and then the script is created. Each program has two types of execution, native and zkvm. In the native execution all the "make a call", sending and receiving of "output of call" and "proposal to state change" are emulated in the intended manner, and the  `CallTape `and `EventTape `is generated. In the zkvm execution,  the actual functions mentioned in the `CallTape ` are executed by corresponding program, and their output is shown to be the same as the ones mentioned in the `CallTape`.'

In this scenario, the `CallTape` would attest to following events

- `usdc_token_program` called `alice_wallet` to execute the `approve_transfer` function with arguments `(usdc_token_object, bob_wallet)`
- `alice_wallet` program approved the transfer, and returned the boolean value `true`
- `usdc_token_program` read the response `true`.

The `EventTape` would attest to the following events

- `usdc_token_program` read `usdc_token_object` from the global state (before seeking approval from wallet program)
- `usdc_token_program` proposed an update to `usdc_token_object` while updating owner's public key, and wallet to that of Bob (after getting approval from the wallet program)

A zkvm execution of both programs produces a proof of their execution. The proof, along with attestation to `CallTape`, confirms that the programs followed the script mentioned in `CallTape`, that is complied with each other's requests and computed the correct function with correct output.
