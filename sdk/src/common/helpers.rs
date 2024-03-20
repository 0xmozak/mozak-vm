use super::types::Poseidon2Hash;

/// Takes leaves of the form `Poseidon2HasType`` and returns the merkle root
/// of the tree, where nodes are hashed according to common prefix of `addr:
/// u64` field. NOTE: Assumes sorted order wrt `addr`
pub fn merkleize(mut addrs: &mut [u64], mut hashes: &mut [Poseidon2Hash]) -> Poseidon2Hash {
    assert_eq!(addrs.len(), hashes.len());
    while addrs.len() > 1 {
        let mut left_read_index = 0;
        let mut curr_addr = addrs[0];
        let mut write_index = 0;
        for right_read_index in 0..addrs.len() {
            let addr = addrs[right_read_index];
            if addr != curr_addr {
                hashes[write_index] =
                    merkleize_group(&mut hashes[left_read_index..right_read_index]);
                addrs[write_index] = curr_addr >> 1;
                left_read_index = right_read_index;
                write_index += 1;
                curr_addr = addr;
            };
        }
        if left_read_index != addrs.len() {
            hashes[write_index] = merkleize_group(&mut hashes[left_read_index..]);
            write_index += 1;
        }
        hashes = &mut hashes[..write_index];
        addrs = &mut addrs[..write_index];
    }
    match hashes.len() {
        0 => Poseidon2Hash::default(),
        _ => hashes[0],
    }
}

#[cfg(target_os = "mozakvm")]
fn merkleize_group(mut group: &mut [Poseidon2Hash]) -> Poseidon2Hash {
    while group.len() > 1 {
        let mut write_index = 0;
        while 2 * write_index + 1 < group.len() {
            let concatenated_node: Vec<u8> = vec![
                group[2 * write_index].inner(),
                group[2 * write_index + 1].inner(),
            ]
            .into_iter()
            .flatten()
            .collect();
            group[write_index] = crate::mozakvm::helpers::poseidon2_hash_no_pad(&concatenated_node);
            write_index += 1;
        }
        if 2 * write_index + 1 == group.len() {
            group[write_index] = group[2 * write_index];
        }
        group = &mut group[..write_index]
    }
    match group.len() {
        0 => Poseidon2Hash::default(),
        _ => group[0],
    }
}

#[cfg(not(target_os = "mozakvm"))]
fn merkleize_group(mut group: &mut [Poseidon2Hash]) -> Poseidon2Hash {
    while group.len() > 1 {
        let mut write_index = 0;
        while 2 * write_index + 1 < group.len() {
            let concatenated_node: Vec<u8> = vec![
                group[2 * write_index].inner(),
                group[2 * write_index + 1].inner(),
            ]
            .into_iter()
            .flatten()
            .collect();
            group[write_index] = crate::native::helpers::poseidon2_hash_no_pad(&concatenated_node);
            write_index += 1;
        }
        if 2 * write_index + 1 == group.len() {
            group[write_index] = group[2 * write_index];
        }
        group = &mut group[..write_index]
    }
    match group.len() {
        0 => Poseidon2Hash::default(),
        _ => group[0],
    }
}
