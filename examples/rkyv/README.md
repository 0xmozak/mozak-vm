
To compile for Mozak-VM use following command:
`cargo +nightly build --release --bin rkyv`

To compile for x86_64 and Linux use following command:
`cargo +nightly build --target x86_64-unknown-linux-gnu --release --bin rkyv-native --features="native"`

