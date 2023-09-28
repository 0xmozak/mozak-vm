
To compile for Mozak-VM use following command:
`cargo +nightly build --release --bin min-max`

To compile for running on your system use following command (kindly change target triple as per your machine's architecture):
`cargo +nightly build --target x86_64-unknown-linux-gnu --release --bin min-max-native --features="native"`
