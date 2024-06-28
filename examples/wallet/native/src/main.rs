//! We approve signatures by asserting the following equality:
//!
//! hash(private_key) == public_key
//!
//! Where `hash` can be any hard to invert function (in this case it's the
//! poseidon2 hash).
//!
//! During native execution:
//! We randomly generate a private key, which we then
//! hash to obtain a public key. We write this private key to our private tape.
//!
//! During guest execution:
//! We read this private key from the private tape and use a poseidon2 ecall to
//! help us prove that we know the pre-image.

// TODO(bing): We may use our `signatures` crate in future as an optimization,
// once we link it to our SDK.
#![allow(unused_attributes)]

use mozak_sdk::common::types::ProgramIdentifier;
use wallet_core_logic::{dispatch, BlackBox, MethodArgs, PrivateKey, PublicKey, TokenObject};
use wallet_elf_data::WALLET_SELF_PROG_ID;

fn main() {
    let wallet_program = ProgramIdentifier::from(WALLET_SELF_PROG_ID.to_string());
    let remitter_program = ProgramIdentifier::new_from_rand_seed(2);
    let remittee_program = ProgramIdentifier::new_from_rand_seed(3);
    let private_key = PrivateKey::new_from_rand_seed(4);
    let public_key = PublicKey(mozak_sdk::native::poseidon::poseidon2_hash_no_pad(
        &private_key.0,
    ));
    mozak_sdk::add_identity(wallet_program); // Manual override for `IdentityStack`
    let _ = mozak_sdk::write(&mozak_sdk::InputTapeType::PrivateTape, &private_key.0[..]);
    mozak_sdk::rm_identity(); // Manual override for `IdentityStack`

    let token_object = TokenObject {
        pub_key: public_key.clone(),
        amount: 10.into(),
    };

    let black_box = BlackBox {
        remitter_program,
        remittee_program,
        token_object,
    };

    mozak_sdk::call_send(
        wallet_program,
        MethodArgs::ApproveSignature(public_key, black_box.clone()),
        dispatch,
    );

    mozak_sdk::native::dump_proving_files();
}
