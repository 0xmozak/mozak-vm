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
On a high level this function is all you need to emulate `caller_prog` asking `callee_prog` to execute `dispatch_native(call_args)`. It also returns the same output that `dispatch_native(call_args)` function would!.
But how does it work?
Remember we said the programs are supposed to show that they have played along the correct script. To understand how this script is generated in the first place, we introduce the notion of **Native execution**. 
It is the native execution which does a "dry run" of the whole programs, along with their interactions, additionally producing the script of the whole run. Apart from producing the script, its almost same as usual rust program execution on user's machine.
Under native execution, `call_send`, in addition to executing `dispatch_native(call_args)`, creates a struct `CPCMessage`
```rust
pub struct CPCMessage {
    pub caller_prog: ProgramIdentifier,
    pub callee_prog: ProgramIdentifier,
    pub call_args: RawMessage,
    // return value of `dispatch_native(call_args)`
    pub ret: RawMessage,
}
```
and appends it to `CallTape`, that is, writes the dialogue of between the programs for the script.
During **ZKVM execution**, `call_send` provably reads the correct `CPCMessage` from the `CallTape`, and returns the `ret` field. That is, provably shows that it is following the movie script as it should.
So much for the details, lets see how we can use in to seek approval from wallet program.

 We would like to call the following `approve_transfer` function from wallet program
```rust
pub fn approve_transfer(
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) -> bool
```
All we have to do is setup the `call_send` function with the arguments.

```rust
// arguments of `approve_transfer` function
let approve_transfer_args = wallet::MethodArgs(token_object, remitter_wallet, remittee_wallet);
// `approve_transfer` function
let approve_function = wallet::approve_transfer;

// `usdc_token` asks `alice_wallet`
// to approve transfer!
let approval = call_send(
    usdc_token,
    alice_wallet,
    approve_transfer_args,
    approve_function
)
```
Thats it!. We know how to a
We can use the same idea to get the `usdc_token_object`, `alice_wallet` and `bob_wallet` as inputs for token program. We would have the function

```rust
pub fn transfer(
    token_program: ProgramIdentifier,
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
)
```
and have the arguments be given through a `call_send` from another program!. For now, we assume alice has written such a program in `alice_program`.

But wait, how would can we now *receive* such a request to call a function?
this time,  can use `call_receive` method provided by sdk

```rust
pub fn call_receive() -> Option<(CPCMessage, usize)>
```
Under native execution, this would return `None`. The call has already been performed, and the dialogue has already been assumed to be written by corresponding `call_send` from other program in this case. So nothing needs to be done.
Under ZKVM execution, however, the program provably reads off the intended `CPCMessage`, and executes the function on the arguments mentioned in the `CPCMessage`. That is, its showing that its playing his role as mentioned in the movie script.

Back to part where `usdc_token_program` needs to receive the arguments to `transfer` from `alice_program`, we can use `call_receive()` in the following manner.

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
