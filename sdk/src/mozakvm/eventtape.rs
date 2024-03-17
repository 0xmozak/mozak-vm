use rkyv::{Archive, Deserialize};

use crate::common::traits::{EventEmit, SelfIdentify};
use crate::common::types::{
    CanonicalEvent, CanonicalOrderedTemporalHints, Event, ProgramIdentifier,
};

/// Represents the `EventTape` under native execution
#[derive(Default, Clone)]
pub struct EventTape {
    pub(crate) self_prog_id: ProgramIdentifier,
    pub(crate) reader: Option<&'static <Vec<CanonicalOrderedTemporalHints> as Archive>::Archived>,
    pub(crate) seen: Vec<bool>,
    pub(crate) index: usize,
}

impl SelfIdentify for EventTape {
    fn set_self_identity(&mut self, id: ProgramIdentifier) { self.self_prog_id = id }

    fn get_self_identity(&self) -> ProgramIdentifier { self.self_prog_id }
}

impl EventEmit for EventTape {
    fn emit(&mut self, event: Event) {
        assert!(self.index < self.reader.unwrap().len());
        let generated_canonical_event = CanonicalEvent::from_event(self.self_prog_id, &event);

        let elem_idx = self.reader.unwrap()[self.index].1 as usize;
        assert!(!self.seen[elem_idx]);
        self.seen[elem_idx] = true;

        let zcd_canonical_event = &self.reader.unwrap()[self.index].0;
        let canonical_event: CanonicalEvent = zcd_canonical_event
            .deserialize(&mut rkyv::Infallible)
            .unwrap();

        assert!(canonical_event == generated_canonical_event);
        self.index += 1;
        // let zcd_event = &self.reader.unwrap()[self.index];
        // let event_deserialized: Event = zcd_event.deserialize(&mut
        // rkyv::Infallible).unwrap();

        // assert!(event == event_deserialized);

        // assert!(
        //     match event.type_ {
        //         EventType::Create | EventType::Delete | EventType::Write =>
        //             event.object.constraint_owner,
        //         _ => self.self_prog_id,
        //     } == self.self_prog_id
        // );

        // self.index += 1;
    }
}
