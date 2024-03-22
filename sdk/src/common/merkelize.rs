use slice_group_by::GroupBy;

use super::types::Poseidon2Hash;
/// Takes leaves of the form `Poseidon2HasType`` and returns the merkle root
/// of the tree, where nodes are hashed according to common prefix of `addr`:
/// u64` field. NOTE: Assumes sorted order wrt `addr`
#[must_use]
pub fn merkleize(mut hashes_with_addr: Vec<(u64, Poseidon2Hash)>) -> Option<Poseidon2Hash> {
    while hashes_with_addr.len() > 1 {
        hashes_with_addr = hashes_with_addr
            .as_slice()
            .linear_group_by(|(addr0, _hash0), (addr1, _hash1)| addr0 == addr1)
            .filter_map(|group| {
                let addr = group.first().copied()?.0;
                let hashes = group.iter().map(|(_, h)| *h).collect::<Vec<_>>();
                Some((addr >> 1, merklelize_group(hashes)?))
            })
            .collect::<Vec<_>>();
    }
    Some(hashes_with_addr.first()?.1)
}

fn merklelize_group(mut group: Vec<Poseidon2Hash>) -> Option<Poseidon2Hash> {
    while group.len() > 1 {
        group = group
            .chunks(2)
            .map(|g| match g {
                [remainder] => *remainder,
                #[cfg(target_os = "mozakvm")]
                g => crate::mozakvm::helpers::poseidon2_hash_no_pad(
                    &g.iter().flat_map(Poseidon2Hash::inner).collect::<Vec<u8>>(),
                ),
                #[cfg(not(target_os = "mozakvm"))]
                g => crate::native::helpers::poseidon2_hash_no_pad(
                    &g.iter().flat_map(Poseidon2Hash::inner).collect::<Vec<u8>>(),
                ),
            })
            .collect::<Vec<_>>();
    }
    group.first().copied()
}
#[cfg(test)]
mod tests {
    use itertools::chain;

    use crate::common::merkelize::merkleize;
    use crate::common::types::Poseidon2Hash;
    use crate::native::helpers::poseidon2_hash_no_pad;

    #[test]
    #[rustfmt::skip] 
    fn merkelize_test() {
        let hashes_with_addr = vec![
            (0x010, Poseidon2Hash([1u8; 32])),// ------------|
                                              //             |--h_2---|  
            (0x011, Poseidon2Hash([2u8; 32])),// ----|       |        |
                                              //     |-h_1---|        |---root
            (0x011, Poseidon2Hash([3u8; 32])),// ----|                |
                                              //                      |
            (0x111, Poseidon2Hash([4u8; 32])),//--------------------- |
        ];
        let h_1 = poseidon2_hash_no_pad(
            &chain![
                hashes_with_addr[1].1.inner(),
                hashes_with_addr[2].1.inner()
            ]
            .collect::<Vec<u8>>(),
        );
        let h_2 = poseidon2_hash_no_pad(
            &chain![hashes_with_addr[0].1.inner(), h_1.inner()]
                .collect::<Vec<u8>>(),
        );
        let root = poseidon2_hash_no_pad(
            &chain![h_2.inner(), hashes_with_addr[3].1.inner()].collect::<Vec<u8>>(),
        );
        assert_eq!(root.inner(), [
            232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192, 
            132, 26, 242, 155, 95, 48, 48, 8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53]);
        assert_eq!(root, merkleize(hashes_with_addr).unwrap());
    }
}
