# Cross Program Communication with `call_send` and `call_receive`

Lets first discuss the part of scenario when `usdc_token_program` needs to seek approval of `alice_wallet` program to do the transfer of `usdc_token_object` to `bob_wallet`. How could such interaction between the two programs could be achieved?

thats where `call_send` function provided by sdk comes in
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
On a high level this function is all you need to emulate `caller_prog` asking `callee_prog` to execute `dispatch_native(call_args)`. It would also provide the return value of the function being called as output. 

In wallet program, we would have
```rust
pub fn approve_transfer(
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) -> bool
```
in the wallet program. Then to emulate the above function call, we do

```rust
let approve_transfer_args = wallet::MethodArgs(token_object, remitter_wallet, remittee_wallet);
let approve_function = wallet::approve_transfer;

let approval = call_send(
    usdc_token,
    alice_wallet,
    approve_transfer_args,
    approve_function
)
```

We can use the same idea to get the `usdc_token_object`, `alice_wallet` and `bob_wallet` as inputs for token program. We can define  a function

```rust
pub fn transfer(
    token_program: ProgramIdentifier,
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
)
```
and have the arguments be given through a `call_send` in another program!. For now, we assume alice has written such a program in `alice_program`.

Next question would be how can we now receive such a request to call a function?
this time,  can use `call_receive` method provided by sdk

```rust
pub fn call_receive() -> Option<(CPCMessage, usize)>
```

The `CPCMessage` struct would all we need to extract the arguments.

```rust
let Some(message_from_alice_program) = call_receive();

let CPCMessage {
    caller_program: alice_program,
    callee_program: token_program,
    args: transfer_call_args,
    ret: transfer_done,
} = message_from_alice_program.0;

let MethodArgs::Transfer(token_program, token_object, alice_wallet, bob_wallet) = transfer_call_args;

transfer(token_program, token_object, alice_wallet, bob_wallet);
```

Similarly in case of wallet program, it would receive the request to approve transfer in the following manner

```rust
pub fn approve_transfer(
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) -> bool

let Some(message_from_token_program) = call_receive();
let CPCMessage {
    token_program,
    wallet_program,
    approve_transfer_args,
    approval,
} = message_from_token_program.0;

let MethodArgs::ApproveTransfer(token_object, alice_wallet, bob_wallet) = approve_call_args;
approve_transfer(token_object, alice_wallet, bob_wallet);

```
