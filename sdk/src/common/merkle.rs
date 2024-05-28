use vec_entries::EntriesExt;

use super::types::Poseidon2Hash;
/// Takes leaves of the form `Poseidon2Hash` and returns the merkle root
/// of the tree, where nodes are hashed according to common prefix of `addr`:
/// `u64` field. NOTE: Assumes sorted order wrt `addr`
#[must_use]
pub fn merkleize(mut hashes_with_addr: Vec<(u64, Poseidon2Hash)>) -> Poseidon2Hash {
    let mut height_incr = 0; // merkleize events at the same address to start
    while hashes_with_addr.len() > 1 {
        height_incr = merkleize_step(&mut hashes_with_addr, height_incr);
    }

    hashes_with_addr.first().map(|x| x.1).unwrap_or_default()
}

// Merkles all the closest relatives once, returns the next merge increment
fn merkleize_step(hashes: &mut Vec<(u64, Poseidon2Hash)>, height_incr: u32) -> u32 {
    let mut next_height_incr = u32::MAX;

    hashes.entries(.., |e| {
        let Some(mut left) = e.remove() else { return };
        left.0 >>= height_incr;

        while let Some(mut right) = e.remove() {
            right.0 >>= height_incr;

            // Combine the two items and insert the result
            if left.0 == right.0 {
                let hash = Poseidon2Hash::two_to_one(left.1, right.1);
                let Ok(_) = e.try_insert_outside((left.0, hash)) else {
                    unreachable!()
                };

                // Make sure to get a new left item
                let Some(next) = e.remove() else { return };
                right = next;
                right.0 >>= height_incr;
            } else {
                let Ok(_) = e.try_insert_outside(left) else {
                    unreachable!()
                };
            }

            // At this point left and right both represent unmerged items
            // See how soon we can merge them by comparing their MSB with XOR
            // and record the lowest
            let height_diff = u64::BITS - (left.0 ^ right.0).leading_zeros();
            next_height_incr = next_height_incr.min(height_diff);

            left = right;
        }

        // Re-insert any unused left items
        let Ok(_) = e.try_insert_outside(left) else {
            unreachable!()
        };
    });

    next_height_incr
}

#[cfg(test)]
mod tests {
    use crate::common::merkle::merkleize;
    use crate::common::types::Poseidon2Hash;
    use crate::core::constants::DIGEST_BYTES;

    #[test]
    #[rustfmt::skip]
    fn merkleize_test() {
        // fn foo() -> bool { true }
        // while foo() {}
        let hashes_with_addr = vec![
            (0x010, Poseidon2Hash([1u8; DIGEST_BYTES])),// ------------|
                                                        //             |--h_2---|
            (0x011, Poseidon2Hash([2u8; DIGEST_BYTES])),// ----|       |        |
                                                        //     |-h_1---|        |---root
            (0x011, Poseidon2Hash([3u8; DIGEST_BYTES])),// ----|                |
                                                        //                      |
            (0x111, Poseidon2Hash([4u8; DIGEST_BYTES])),//--------------------- |
        ];
        let h_1 = Poseidon2Hash::two_to_one(
            hashes_with_addr[1].1,
            hashes_with_addr[2].1,
        );
        let h_2 = Poseidon2Hash::two_to_one(
            hashes_with_addr[0].1, h_1
        );
        let root = Poseidon2Hash::two_to_one(
            h_2, hashes_with_addr[3].1,
        );
        assert_eq!(root.inner(), [
            232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192,
            132, 26, 242, 155, 95, 48, 48, 8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53]);
        assert_eq!(root, merkleize(hashes_with_addr));
    }
}
