use im::hashmap::HashMap;

#[must_use]
pub fn load_u32(m: &HashMap<u32, u8>, addr: u32) -> u32 {
    const WORD_SIZE: usize = 4;
    let mut bytes = [0_u8; WORD_SIZE];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = m.get(&(addr + i as u32)).copied().unwrap_or_default();
    }
    u32::from_le_bytes(bytes)
}
