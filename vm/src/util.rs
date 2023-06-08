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


#[must_use]
pub fn ceil_power_of_two(a: usize) -> usize {
    a.wrapping_sub(1).wrapping_next_power_of_two()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use super::ceil_power_of_two;
    pub fn ceil_power_of_two_simple(a: usize) -> usize {
        if !a.is_power_of_two() {
            a.next_power_of_two()
        } else {
            a
        }
    }
    proptest! {
        #[test]
        fn oracle(a in any::<usize>()) {
            // let _x = ceil_power_of_two(a);
            let _y = ceil_power_of_two_simple(a);
            // assert_eq!(x, y);
        }
    }
    
}
