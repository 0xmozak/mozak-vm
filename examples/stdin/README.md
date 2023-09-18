
To compile for Mozak-VM use following command:
`cargo +nightly build --release --bin stdin`

To compile for x86_64 and Linux use following command:
`cargo +nightly build --target x86_64-unknown-linux-gnu --release --bin stdin-x86 --features="x86"`


First execute `stdin-x86` which will create `iotape.txt` file. Next run `stdin` which will read input from `iotape.txt` file.

