## High-Level Overview of a Transaction


**Token Wallet Example:**

Imagine Alice owns USDC tokens in her wallet. She wants to transfer them to Bob's wallet. This transfer is done by the USDC token program, which can only modify USDC tokens after receiving approval from the owner's wallet program (Alice's in this case).

**Program-Level Breakdown:**

A USDC token is represented by a `StateObject` with a unique owner (`USDC token program`) and details like Alice's ownership and wallet program. The USDC program requires approval from Alice's wallet program to transfer ownership to Bob and his wallet program.

```rust
let alice_wallet: ProgramIdentifier;
let bob_wallet: ProgramIdentifier;

let usdc_token_data = USDCTokenData {
    usdc_token_owner: ALICE_PUBLIC_KEY,
    wallet_program: alice_wallet,
    amount: 100,
};

let usdc_token_program: ProgramIdentifier;

let usdc_token_object = StateObject {
    address: address_of_object_in_state_tree,
    constraint_owner: usdc_token_program,
    data: usdc_token_data.to_bytes(),
};
```

At a high level, the program performs the following:

1. `usdc_token_program` verifies Alice owns `usdc_token_object`.
2. It requests `alice_wallet` to "call" a function approving the transfer of `usdc_token_object` to `bob_wallet`.
3. `alice_wallet` "receives the request" and calls its approval function.
4. The approval function proves Alice possesses the pre-image hash of the public key (essentially, her private key) for `usdc_token_object`. This proof is provided as private input to the wallet program.
5. `usdc_token_program` receives the approval.
6. It then "proposes a change" to update `usdc_token_object`'s owner and wallet to Bob's.

Each program generates a zero-knowledge proof of its execution within zkVM. However, this isn't enough! The proof needs to capture the communication and interaction with the global state between programs.

**Cross-Program Communication (CPC):**

Programs don't actually "call" or "receive responses" in the traditional sense. Instead, they demonstrate following a predetermined script defining their interactions. This script, called `CallTape`, ensures both programs adhere to the agreed-upon communication protocol. Our proof system verifies the script's execution.

For example, when the token program requests approval from Alice's wallet program, `CallTape` would record the following event:

- `usdc_token_program` called `alice_wallet` to execute `approve_transfer` with arguments `(usdc_token_object, bob_wallet)`.
- `alice_wallet` approved the transfer, returning `true`.

**Provable State Interaction:**

Similar to `CallTape`, a `EventTape` stores all reads and writes a program performs on the global state. The program's proof also verifies the events on `EventTape` that it claims to have emitted.

For instance, when the `usdc_token_program` wants to transfer ownership of `usdc_token_object` to Bob, `EventTape` would record:

- `usdc_token_program` proposed an update to `usdc_token_object`'s `owner` field to `BOB_PUBLIC_KEY` and `wallet` field to `bob_wallet`.

**Script Generation:**

The script is essentially created by performing a "dry run" of all programs, mimicking their regular execution as Rust programs. This is called **Native Execution**. During this execution, the script is generated, and all cross-program calls and global state interactions are recorded in `CallTape` and `EventTape`, respectively.

**Following the Script:**

We have another execution environment called **ZKVM Execution** for generating proofs. After preparing the tapes, programs are run individually on zkVM. When a program needs to communicate with another, it reads from `CallTape` and provably accesses the agreed-upon response. It then continues its execution based on the response.

**TODO:** Transaction Bundle
