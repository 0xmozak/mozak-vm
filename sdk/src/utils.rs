use crate::coretypes::Poseidon2HashType;
use crate::sys::poseidon2_hash_no_pad;

#[must_use]
/// Takes vector of leaves of the form (address, hash) sorted according to
/// address, and returns root of corresponding merkle tree.
pub fn merklelize(mut hashes_with_addr: Vec<(u64, Poseidon2HashType)>) -> Poseidon2HashType {
    while hashes_with_addr.len() > 1 {
        let mut new_hashes_with_addr = vec![];
        let mut i = 0;
        while i < hashes_with_addr.len() {
            let (left_addr, left_hash) = hashes_with_addr[i];
            if i == hashes_with_addr.len() - 1 {
                new_hashes_with_addr.push((left_addr >> 1, left_hash));
                break;
            }
            let (right_addr, right_hash) = hashes_with_addr[i + 1];
            if left_addr == right_addr {
                new_hashes_with_addr.push((
                    left_addr >> 1,
                    poseidon2_hash_no_pad(
                        &vec![left_hash.to_le_bytes(), right_hash.to_le_bytes()]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<u8>>(),
                    ),
                ));
                i += 2;
            } else {
                new_hashes_with_addr.push((left_addr >> 1, left_hash));
                i += 1;
            }
        }
        hashes_with_addr = new_hashes_with_addr;
    }
    let (_root_addr, root_hash) = hashes_with_addr[0];
    root_hash
}

#[cfg(test)]
mod tests {

    use itertools::chain;

    use super::merklelize;
    use crate::coretypes::{
        Address, CanonicalEventType, Event, Poseidon2HashType, ProgramIdentifier, StateObject,
    };
    use crate::sys::{poseidon2_hash_no_pad, CanonicalEventTapeSingle, EventTapeSingle};

    #[test]
    pub fn sample_test_run() {
        let program_id = ProgramIdentifier::default();
        let object = StateObject {
            address: Address::from([1u8; 8]),
            constraint_owner: ProgramIdentifier::default(),
            data: vec![1, 2, 3, 4, 5],
        };

        let new_object = StateObject {
            data: vec![6, 7, 8, 9, 10],
            ..object
        };

        let another_object = StateObject {
            address: Address::from([2u8; 8]),
            constraint_owner: ProgramIdentifier::default(),
            data: vec![1, 2, 3, 4, 5, 6],
        };

        let read_event = Event {
            object,
            operation: CanonicalEventType::Read,
        };

        let write_event = Event {
            object: new_object,
            operation: CanonicalEventType::Write,
        };

        let another_object_read_event = Event {
            object: another_object,
            operation: CanonicalEventType::Read,
        };

        let event_tape = EventTapeSingle {
            id: program_id,
            contents: vec![read_event, write_event, another_object_read_event],
            canonical_repr: Default::default(),
        };

        let canonical_event_tape: CanonicalEventTapeSingle = event_tape.into();
        let root_hash = canonical_event_tape.canonical_hash();
        assert_eq!(root_hash.to_le_bytes(), [
            145, 36, 249, 45, 165, 207, 199, 178, 237, 63, 61, 119, 154, 69, 157, 172, 212, 0, 178,
            143, 174, 36, 139, 46, 174, 198, 15, 225, 228, 164, 117, 169
        ])
    }
    #[test]
    fn merkelize_test() {
        let hashes_with_addr = vec![
            (0x010, Poseidon2HashType([1u8; 32])),
            (0x011, Poseidon2HashType([2u8; 32])),
            (0x011, Poseidon2HashType([3u8; 32])),
            (0x111, Poseidon2HashType([4u8; 32])),
        ];
        let hash_12 = poseidon2_hash_no_pad(
            &chain![
                hashes_with_addr[1].1.to_le_bytes(),
                hashes_with_addr[2].1.to_le_bytes()
            ]
            .collect::<Vec<u8>>(),
        );
        let hash_1 = poseidon2_hash_no_pad(
            &chain![hashes_with_addr[0].1.to_le_bytes(), hash_12.to_le_bytes()]
                .collect::<Vec<u8>>(),
        );
        let hash_13 = poseidon2_hash_no_pad(
            &chain![hash_1.to_le_bytes(), hashes_with_addr[3].1.to_le_bytes()].collect::<Vec<u8>>(),
        );
        assert_eq!(hash_13, merklelize(hashes_with_addr));
    }
}
