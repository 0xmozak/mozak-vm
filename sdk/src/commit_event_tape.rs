use crate::coretypes::{CanonicalEvent, Poseidon2HashType};
use crate::sys::{poseidon2_hash_no_pad, poseidon2_hash_with_pad, CanonicalEventTapeSingle};

#[allow(unused)]
pub fn hash_canonical_event(event: &CanonicalEvent) -> Poseidon2HashType {
    poseidon2_hash_with_pad(
        &vec![
            event.address.to_le_bytes().to_vec(),
            vec![event.event_type.clone() as u8],
            event.event_value.0.to_vec(),
            event.event_emitter.0.to_vec(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>(),
    )
}

#[allow(unused)]
pub fn hash_canonical_event_tape(tape: CanonicalEventTapeSingle) -> Poseidon2HashType {
    // collect hashes
    let mut hashes_with_addr = tape
        .sorted_events
        .iter()
        .map(|event| (event.address, hash_canonical_event(event)))
        .collect::<Vec<(u32, Poseidon2HashType)>>();

    while hashes_with_addr.len() > 1 {
        let mut new_hashes_with_addr = vec![];
        let mut prev_pair = None;
        for (mut current_addr, current_hash) in hashes_with_addr {
            match prev_pair {
                None => prev_pair = Some((current_addr, current_hash)),
                Some((mut prev_addr, prev_hash)) => {
                    current_addr = current_addr >> 1;
                    prev_addr = prev_addr >> 1;
                    if prev_addr == current_addr {
                        new_hashes_with_addr.push((
                            current_addr,
                            poseidon2_hash_no_pad(
                                &(vec![
                                    current_hash.to_le_bytes().to_vec(),
                                    prev_hash.to_le_bytes().to_vec(),
                                ])
                                .into_iter()
                                .flatten()
                                .collect::<Vec<u8>>(),
                            ),
                        ));
                    } else {
                        new_hashes_with_addr
                            .extend(vec![(prev_addr, prev_hash), (current_addr, current_hash)])
                    }
                    prev_pair = None;
                }
            }
        }
        hashes_with_addr = new_hashes_with_addr;
    }
    let (_root_addr, root_hash) = hashes_with_addr[0];
    root_hash
}

#[cfg(test)]
mod tests {
    use super::hash_canonical_event_tape;
    use crate::coretypes::{Address, CanonicalEventType, Event, ProgramIdentifier, StateObject};
    use crate::sys::{CanonicalEventTapeSingle, EventTapeSingle};

    #[test]
    pub fn sample_test_run() {
        let program_id = ProgramIdentifier::default();
        let object = StateObject {
            address: Address::from([1u8; 4]),
            constraint_owner: ProgramIdentifier::default(),
            data: vec![1, 2, 3, 4, 5],
        };

        let new_object = StateObject {
            data: vec![6, 7, 8, 9, 10],
            ..object
        };

        let read_event = Event {
            object,
            operation: crate::coretypes::CanonicalEventType::Read,
        };

        let write_event = Event {
            object: new_object,
            operation: CanonicalEventType::Write,
        };

        let event_tape = EventTapeSingle {
            id: program_id,
            contents: vec![read_event, write_event],
            canonical_repr: Default::default(),
        };

        let canonical_event_tape: CanonicalEventTapeSingle = event_tape.into();
        let _root_hash = hash_canonical_event_tape(canonical_event_tape);
    }
}
