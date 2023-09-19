
To compile for Mozak-VM use following command:
`cargo +nightly build --release --bin stdin`

To compile for running on your system use following command (kindly change target triple as per your machine's architecture):
`cargo +nightly build --target x86_64-unknown-linux-gnu --release --bin stdin-native --features="native"`


First execute `stdin-native` which will create `iotape.txt` file. Next run `stdin` which will read input from `iotape.txt` file.

