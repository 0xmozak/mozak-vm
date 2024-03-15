use crate::coretypes::Poseidon2HashType;
use crate::sys::poseidon2_hash_no_pad;

#[must_use]
/// Takes vector of leaves of the form (address, hash) sorted according to
/// address, and returns root of corresponding merkle tree.
/// It works in following fashion.
pub fn merklelize(hashes_with_addr: &[(u64, Poseidon2HashType)]) -> Poseidon2HashType {
    match hashes_with_addr.len() {
        0 => panic!("Didn't expect 0"),
        1 => hashes_with_addr[0].1,
        _ => merklelize(
            &hashes_with_addr
                .group_by(|(addr0, _), (addr1, _)| addr0 == addr1)
                .map(|group| {
                    let addr = group.first().copied().unwrap_or_default().0;
                    let hashes: Vec<Poseidon2HashType> =
                        group.iter().map(|(_, h)| *h).collect::<Vec<_>>();
                    (addr >> 1, merklelize_group(&hashes))
                })
                .collect::<Vec<_>>(),
        ),
    }
}

fn merklelize_group(group: &[Poseidon2HashType]) -> Poseidon2HashType {
    match group.len() {
        0 => panic!("Didn't expect 0"),
        1 => group[0],
        _ => merklelize_group(
            &group
                .chunks(2)
                .map(|g| match g {
                    [remainder] => *remainder,
                    g => poseidon2_hash_no_pad(
                        &g.iter()
                            .flat_map(Poseidon2HashType::to_le_bytes)
                            .collect::<Vec<u8>>(),
                    ),
                })
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(test)]
mod tests {

    use itertools::chain;

    use super::merklelize;
    use crate::coretypes::{
        Address, CanonicalEvent, CanonicalEventType, Event, Poseidon2HashType, ProgramIdentifier,
        StateObject,
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
            79, 14, 176, 199, 183, 132, 30, 42, 230, 153, 157, 39, 200, 196, 161, 42, 143, 239,
            246, 55, 106, 106, 211, 0, 8, 102, 73, 157, 46, 176, 198, 26
        ])
    }
    #[test]
    #[rustfmt::skip] 
    fn merkelize_test() {
        let hashes_with_addr = vec![
            (0x010, Poseidon2HashType([1u8; 32])),// ------------|
                                                  //             |--h_2---|  
            (0x011, Poseidon2HashType([2u8; 32])),// ----|       |        |
                                                  //     |-h_1---|        |---root
            (0x011, Poseidon2HashType([3u8; 32])),// ----|                |
                                                  //                      |
            (0x111, Poseidon2HashType([4u8; 32])),//--------------------- |
        ];
        let h_1 = poseidon2_hash_no_pad(
            &chain![
                hashes_with_addr[1].1.to_le_bytes(),
                hashes_with_addr[2].1.to_le_bytes()
            ]
            .collect::<Vec<u8>>(),
        );
        let h_2 = poseidon2_hash_no_pad(
            &chain![hashes_with_addr[0].1.to_le_bytes(), h_1.to_le_bytes()]
                .collect::<Vec<u8>>(),
        );
        let root = poseidon2_hash_no_pad(
            &chain![h_2.to_le_bytes(), hashes_with_addr[3].1.to_le_bytes()].collect::<Vec<u8>>(),
        );
        assert_eq!(root.to_le_bytes(), [
            232, 132, 143, 27, 162, 220, 25, 57, 138, 30, 151, 109, 192, 
            132, 26, 242, 155, 95, 48, 48, 8, 55, 240, 62, 54, 195, 137, 239, 231, 140, 205, 53]);
        assert_eq!(root, merklelize(hashes_with_addr));
    }

    fn hashout_to_bytes_hash(hashout: [u64; 4]) -> [u8; 32] {
        hashout
            .to_vec()
            .into_iter()
            .flat_map(|limb| limb.to_le_bytes())
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap()
    }

    #[test]
    fn check_sample_events_hash() {
        let hash_1 = hashout_to_bytes_hash([4, 8, 15, 16]);
        let zero_val = hashout_to_bytes_hash([0, 0, 0, 0]);
        let non_zero_val_1 = Poseidon2HashType(hashout_to_bytes_hash([3, 1, 4, 15]));
        let non_zero_val_2 = Poseidon2HashType(hashout_to_bytes_hash([1, 6, 180, 33]));
        let program_hash_1 = ProgramIdentifier(Poseidon2HashType(hash_1));
        let zero_val_hash = Poseidon2HashType(zero_val);
        let read_0 = CanonicalEvent {
            address: 42,
            event_owner: program_hash_1,
            event_value: zero_val_hash,
            event_type: CanonicalEventType::Read,
        };

        let write_1 = CanonicalEvent {
            address: 42,
            event_owner: program_hash_1,
            event_type: CanonicalEventType::Write,
            event_value: non_zero_val_1,
        };
        let write_2 = CanonicalEvent {
            address: 42,
            event_owner: program_hash_1,
            event_type: CanonicalEventType::Write,
            event_value: non_zero_val_2,
        };
        const READ_0_HASH: [u64; 4] = [
            7272290939186032751,
            8185818005188304227,
            17555306369107993266,
            17187284268557234321,
        ];

        const WRITE_1_HASH: [u64; 4] = [
            11469795294276139037,
            799622748573506082,
            15272809121316752941,
            7142640452443475716,
        ];
        const WRITE_2_HASH: [u64; 4] = [
            1484423020241144842,
            17207848040428508675,
            7995793996020726058,
            4658801606188332384,
        ];

        const BRANCH_1_HASH: [u64; 4] = [
            16758566829994364981,
            15311795646108582705,
            12773152691662485878,
            2551708493265210224,
        ];
        const BRANCH_2_HASH: [u64; 4] = [
            8577138257922146843,
            5112874340235798754,
            4121828782781403483,
            12250937462246573507,
        ];

        assert_eq!(
            hashout_to_bytes_hash(READ_0_HASH),
            read_0.canonical_hash().to_le_bytes()
        );
        assert_eq!(
            hashout_to_bytes_hash(WRITE_1_HASH),
            write_1.canonical_hash().to_le_bytes()
        );
        assert_eq!(
            hashout_to_bytes_hash(WRITE_2_HASH),
            write_2.canonical_hash().to_le_bytes()
        );

        assert_eq!(
            hashout_to_bytes_hash(BRANCH_1_HASH),
            poseidon2_hash_no_pad(
                &chain!(
                    write_1.canonical_hash().to_le_bytes(),
                    write_2.canonical_hash().to_le_bytes()
                )
                .collect::<Vec<u8>>()
            )
            .to_le_bytes()
        );

        assert_eq!(
            hashout_to_bytes_hash(BRANCH_2_HASH),
            poseidon2_hash_no_pad(
                &chain!(
                    read_0.canonical_hash().to_le_bytes(),
                    hashout_to_bytes_hash(BRANCH_1_HASH)
                )
                .collect::<Vec<u8>>()
            )
            .to_le_bytes()
        )
    }
}
