pub fn min_max() -> Vec<u8> {
    let min = std::cmp::min(100_u32, 1000_u32);
    let max = std::cmp::max(100_u32, 1000_u32);
    assert!(min < max);
    max.to_be_bytes().to_vec()
}
