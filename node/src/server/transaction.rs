struct Transaction {
    data: Vec<u8>,
}

impl Transaction {
    fn dummy() -> Self { Self { data: vec![0; 32] } }
}
