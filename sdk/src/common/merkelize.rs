use super::types::Poseidon2Hash;

#[cfg(not(target_os = "mozakvm"))]
const POSEIDON2_HASH_NO_PAD: fn(&[u8]) -> Poseidon2Hash =
    crate::native::helpers::poseidon2_hash_no_pad;
#[cfg(target_os = "mozakvm")]
const POSEIDON2_HASH_NO_PAD: fn(&[u8]) -> Poseidon2Hash =
    crate::mozakvm::helpers::poseidon2_hash_no_pad;

fn hash_top_two(mut stack: Vec<Poseidon2Hash>) -> Vec<Poseidon2Hash> {
    let concatenated_node = [stack.pop().unwrap().inner(), stack.pop().unwrap().inner()].concat();

    stack.push(POSEIDON2_HASH_NO_PAD(&concatenated_node));
    stack
}

pub fn merkleize_with_hints(hashes: &[(Poseidon2Hash, u8)]) -> Poseidon2Hash {
    let mut stack: Vec<Poseidon2Hash> = Vec::new();
    for (hash, pops) in hashes {
        stack.push(*hash);
        for _ in 0..*pops {
            stack = hash_top_two(stack);
        }
    }
    while stack.len() > 1 {
        stack = hash_top_two(stack);
    }
    stack.pop().unwrap()
}

/// Takes leaves of the form `Poseidon2HasType`` and returns the merkle root
/// of the tree, where nodes are hashed according to common prefix of `addr`:
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
        hashes[write_index] = merkleize_group(&mut hashes[left_read_index..]);
        addrs[write_index] = curr_addr >> 1;
        write_index += 1;

        hashes = &mut hashes[..write_index];
        addrs = &mut addrs[..write_index];
    }
    match hashes.len() {
        0 => Poseidon2Hash::default(),
        _ => hashes[0],
    }
}

pub fn merkleize_group(mut group: &mut [Poseidon2Hash]) -> Poseidon2Hash {
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
            #[cfg(not(target_os = "mozakvm"))]
            {
                group[write_index] =
                    crate::native::helpers::poseidon2_hash_no_pad(&concatenated_node);
            }
            #[cfg(target_os = "mozakvm")]
            {
                group[write_index] =
                    crate::mozakvm::helpers::poseidon2_hash_no_pad(&concatenated_node);
            }
            write_index += 1;
        }
        if 2 * write_index + 1 == group.len() {
            group[write_index] = group[2 * write_index];
            write_index += 1;
        }
        group = &mut group[..write_index];
    }
    match group.len() {
        0 => Poseidon2Hash::default(),
        _ => group[0],
    }
}

#[cfg(test)]
mod tests {
    use itertools::chain;

    use crate::common::merkelize::{merkleize, merkleize_with_hints};
    use crate::common::types::Poseidon2Hash;
    use crate::native::helpers::poseidon2_hash_no_pad;

    #[test]
    #[rustfmt::skip] 
    fn merkelize_test() {
        let mut addr = vec![
            0x010, // ------------|
                   //             |--h_2---|  
            0x011, // ----|       |        |
                   //     |-h_1---|        |---root
            0x011, // ----|                |
                   //                      |
            0x111, //--------------------- |
        ];
        let mut hashes = vec![
            Poseidon2Hash([1u8; 32]),
            Poseidon2Hash([2u8; 32]),
            Poseidon2Hash([3u8; 32]),
            Poseidon2Hash([4u8; 32])
        ];
        let h_1_pre_image: Vec<u8> = chain![
            hashes[1].inner(),
            hashes[2].inner()
        ].collect();
        let h_1 = poseidon2_hash_no_pad(
            &h_1_pre_image
        );
        let h_2_pre_image: Vec<u8> = chain![hashes[0].inner(), h_1.inner()]
        .collect();
        let h_2 = poseidon2_hash_no_pad(
            &h_2_pre_image,
        );
        let root_pre_image: Vec<u8> = chain![h_2.inner(), hashes[3].inner()].collect();
        let root = poseidon2_hash_no_pad(
            &root_pre_image,
        );
        assert_eq!(root.inner(), [
            232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192, 
            132, 26, 242, 155, 95, 48, 48, 8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53]);
        assert_eq!(root, merkleize(&mut addr, &mut hashes));
    }

    // TODO: write a better test.
    #[test]
    fn merkelize_with_hints_test() {
        let hashes_with_hints = vec![
            (Poseidon2Hash([4u8; 32]), 0),
            (Poseidon2Hash([3u8; 32]), 0),
            (Poseidon2Hash([2u8; 32]), 1),
            (Poseidon2Hash([1u8; 32]), 2),
        ];
        let root = merkleize_with_hints(&hashes_with_hints);
        assert_eq!(
            [
                232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192, 132, 26, 242, 155, 95,
                48, 48, 8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53
            ],
            root.inner()
        );
    }
}
