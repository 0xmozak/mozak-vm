```rust
pub fn main() {
    // call from alice's program
    if let Some(message_from_alice) = call_receive() {
        let CPCMessage {
            caller_program: alice_program,
            callee_program: token_program,
            args: transfer_call_args,
            ret: transfer_done,
        } = message_from_null_to_token.0;

        // arguments supplied by alice
        // id: token_program
        // token_object: USDC token owned by alice
        // remitter: alice's wallet program
        // remittee: bob's wallet program
        let MethodArgs::Transfer(id, token_object, alice_wallet, bob_wallet) =
            transfer_call_args;

        // assert that token object is owned by the token program
        assert_eq!(token_program, token_object.constraint_owner);

        let token_object_data: TokenData = token_object.data.into();
        
        // assert that alice's wallet is one of the 
        // economic owner of token_object
        assert_eq!(token_object_data.wallet, alice_wallet);

        let alice_public_key = token_object_data.owner;

        transfer(id, token_object, alice_wallet, bob_wallet, alice_public_key);
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
        remittee_public_key
        (token_object,
        remittee_wallet),
    );
    let approve_transfer_return = wallet::MethodReturns::ApproveTransfer(true);

    assert_eq!(call_send(
        token_program,
        remitter_wallet,
        approve_transfer_args,
        native_approve_transfer,
    ), approve_transfer_return);

    // create the new object
    let new_token_object = StateObject {
        data: TokenData {
            owner: remittee_public_key,
            wallet: remittee_wallet,
            amount: token_object.data.amount,
        },
        ..token_object
    };

    event_emit(self_prog_id, Event::UpdatedStateObject(new_token_object));
}


```
