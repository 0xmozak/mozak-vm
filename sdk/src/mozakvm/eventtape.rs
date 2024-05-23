use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize};

use crate::common::traits::{EventEmit, SelfIdentify};
use crate::common::types::{
    CanonicalEvent, CanonicallyOrderedEventsWithTemporalHints, Event, Poseidon2Hash,
    ProgramIdentifier,
};

/// Represents the `EventTape` under native execution
#[derive(Default, Clone)]
pub struct EventTape {
    pub(crate) self_prog_id: ProgramIdentifier,
    pub(crate) reader:
        Option<&'static <Vec<CanonicallyOrderedEventsWithTemporalHints> as Archive>::Archived>,
    pub(crate) seen: Vec<bool>,
    pub(crate) index: usize,
}

impl SelfIdentify for EventTape {
    fn set_self_identity(&mut self, id: ProgramIdentifier) { self.self_prog_id = id }

    fn get_self_identity(&self) -> ProgramIdentifier { self.self_prog_id }
}

impl EventEmit for EventTape {
    fn emit(&mut self, event: Event) {
        assert!(self.index < self.reader.as_ref().unwrap().len());
        let generated_canonical_event = CanonicalEvent::from_event(&event);
        let elem_idx: usize = self.reader.unwrap()[self.index]
            .1
            .to_native()
            .try_into()
            .unwrap();
        assert!(!self.seen[elem_idx]);
        self.seen[elem_idx] = true;

        let zcd_canonical_event = &self.reader.as_ref().unwrap()[elem_idx].0;
        let canonical_event: CanonicalEvent = zcd_canonical_event
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();

        assert!(canonical_event == generated_canonical_event);
        self.index += 1;
    }
}

impl EventTape {
    pub fn canonical_hash(&self) -> Poseidon2Hash {
        let vec_canonical_ordered_temporal_hints: Vec<CanonicallyOrderedEventsWithTemporalHints> =
            self.reader
                .unwrap()
                .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                .unwrap();
        let vec_canonical_event: Vec<CanonicalEvent> = vec_canonical_ordered_temporal_hints
            .iter()
            .map(|event| event.0)
            .collect();
        let hashes_with_addr: Vec<(u64, Poseidon2Hash)> = vec_canonical_event
            .iter()
            .map(|event| {
                (
                    u64::from_le_bytes(event.address.inner()),
                    event.canonical_hash(),
                )
            })
            .collect();
        crate::common::merkle::merkleize(hashes_with_addr)
    }
}
