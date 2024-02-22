# Wallet program

```rust
pub fn main() {
    if let Some(message_from_token_program) = call_receive() {
        let CPCMessage {
            token_program,
            wallet_program,
            approve_call_args,
            approval,
        } = message_from_token_program.0;

        let MethodArgs::ApproveTransfer(token_object, alice_wallet, bob_wallet) = approve_call_args;

        assert_eq!(
            approval,
            approve_transfer(token_object, alice_wallet, bob_wallet)
        )
    } else {
        panic!("Failed to receive message from token program")
    }
}

pub fn approve_transfer(
    token_object: StateObject,
    remitter_wallet: ProgramIdentifier,
    remittee_wallet: ProgramIdentifier,
) -> bool {
    let (mut public_tape, mut private_tape) = get_tapes();
    let private_key = PrivateKey::from(private_tape);
    let token_object_data = token_object.data.into();
    let public_key = token_object_data.public_key;
    private_key == poseidon2_hash(public_key)
}

```
