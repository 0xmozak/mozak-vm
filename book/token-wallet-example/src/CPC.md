## Cross-Program Communication with `call_send` and `call_receive`
The SDK offers two powerful functions, `call_send` and `call_receive`, that help us to emulate communication between token and wallet program.

### Understanding `call_send`
`call_send` allows a program (the caller) to invoke a function from another program (the callee). 
```rust
pub fn call_send<A, R>(
    caller_prog: ProgramIdentifier,
    callee_prog: ProgramIdentifier,
    call_args: A,
    dispatch_native: impl Fn(A) -> R,
) -> R
where
    A: CallArgument,
    R: CallReturn, {}
```
To the developer, it behaves in same way as the code line
```rust
let output = dispatch_native(call_args)
```
While behind the scenes, during the "dry run", that is, the native execution, `call_send` would create a `CPCMessage` structure. This structure captures information about the call, including the caller and callee program identifiers, the function arguments, and the expected return value.

```rust
pub struct CPCMessage {
    pub caller_prog: ProgramIdentifier,
    pub callee_prog: ProgramIdentifier,
    pub call_args: RawMessage,
    // return value of `dispatch_native(call_args)`
    pub ret: RawMessage,
}
```
This structure would be appended to `CallTape`, essentially recording the dialogue between the programs as defined by the script.

During the zkvm execution, `call_send` essentially reads off from the correct `CPCMessage` from the  `CallTape` and extracts the `ret` value. This corresponds to initiating the dialogue, and acknowledging that it has received the response.

### Usage in requesting wallet for approval

Consider a scenario where a `usdc_token` program needs permission from an `alice_wallet` program to transfer USDC tokens to bob_wallet. Here's how `call_send` comes into play:

```rust

// Arguments for `approve_transfer` function in `alice_wallet`
let approve_transfer_args = wallet::MethodArgs(token_object, remitter_wallet, remittee_wallet);

// Function to call in `alice_wallet`
let approve_function = wallet::approve_transfer;

// `usdc_token` calls `alice_wallet` to request approval
let approval = call_send(
    usdc_token,
    alice_wallet,
    approve_transfer_args,
    approve_function
);
```
This example demonstrates how `usdc_token` uses `call_send` to invoke the `approve_transfer` function within `alice_wallet`, passing the necessary arguments and receiving the approval status.

### Understanding `call_receive`

The `call_receive` function complements `call_send` by enabling the callee program to receive and process the call initiated by the caller.

```rust
pub fn call_receive() -> Option<(CPCMessage, usize)>
```

During native execution, `call_receive` acts as a placeholder, returning `None` since the corresponding `call_send` is assumed to have already recorded the dialougue.

During ZKVM execution, the program provably reads the relevant `CPCMessage` from the `CallTape`, extracting the call details and arguments. Based on the extracted information, the program executes the designated function with the provided arguments. That is, provably showing that it is following the instructions mentioned in the dialouge.

### Usage in receiving the request to approve

Continuing from the token transfer scenario, here's how `alice_wallet` uses `call_receive` to handle the request from `usdc_token`:


```rust
// `alice_wallet` receives request for approval
// from `usdc_token_program`
let Some(message_from_token_program) = call_receive();

// the request is decoded
let CPCMessage {
    caller_program: token_program,
    callee_program: wallet_program,
    approve_transfer_args,
    approval,
} = message_from_token_program.0;

// the arguments to `approve_transfer` call are 
// prepared
let MethodArgs::ApproveTransfer(token_object, alice_wallet, bob_wallet) = approve_call_args;

// the wallet calls its internal function
// to approve the transfer
approve_transfer(token_object, alice_wallet, bob_wallet);
```
