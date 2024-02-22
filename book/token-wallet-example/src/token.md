# Token program

```rust


pub fn main() {
    // signals initiation of program
    // Null program in some sense, provides the arguments
    // to the token program, which are supplied by Alice
    if let Some(message_from_null_to_token) = call_receive() {
        let CPCMessage {
            caller_program: null_program,
            callee_program: token_program,
            args: transfer_call_args,
            ret: transfer_done,
        } = message_from_null_to_token.0;

        if null_program != ProgramIdentifier::default() {
            panic!("Caller is not the null program");
        };

        // arguments supplied by alice
        // id: token_program
        // token_object: USDC token owned by alice
        // remitter: alice's wallet program
        // remittee: bob's wallet program
        // remittee_public_key: bob's public key
        let MethodArgs::Transfer(id, token_object, alice_wallet, bob_wallet, bob_public_key) =
            transfer_call_args;

        // assert that token object is owned by the token program
        assert_eq!(token_program, token_object.constraint_owner);

        // token program calls its transfer function on args supplied by Alice
        let success = transfer(id, token_object, alice_wallet, bob_wallet);

        if success = transfer_done {
            panic!("Transfer failed");
        }
    } else {
        panic!("No One called the program");
    }
}

pub fn transfer(
    token_program: ProgramIdentifier,
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
    remittee_public_key: PublicKey,
) -> bool {
    // set up the args for approve_transfer function to be called by wallet program
    let approve_transfer_args = Wallet::MethodArgs::ApproveTransfer(
        token_object,
        remitter_wallet,
        remittee_wallet,
        remittee_public_key,
    );
    let approve_transfer_return = wallet::MethodReturns::ApproveTransfer(true);

    // the native function that is run by wallet program
    // This would be executed during native execution
    // and facilitate generation of calltape
    let native_approve_transfer = Wallet::approve_transfer;

    let wallet::MethodReturns::ApproveTransfer(success) = call_send(
        token_program,
        remitter_wallet,
        approve_transfer_args,
        native_approve_transfer,
        approve_transfer_return,
    );

    if success == approve_transfer_return {
        let new_token_object = StateObject {
            data: TokenData {
                owner: remittee_public_key,
                wallet: remittee_wallet,
                amount: token_object.data.amount,
            },
        };

        event_emit(self_prog_id, Event::UpdatedStateObject(new_token_object));
    }
}

```
