use rkyv::{Archive, Deserialize};

use crate::traits::{EventEmit, SelfIdentify};
use crate::types::{CanonicalEventType, Event, ProgramIdentifier};

/// Represents the `EventTape` under native execution
#[derive(Default)]
pub struct EventTapeMozak {
    pub self_prog_id: ProgramIdentifier,
    pub reader: Option<&'static <Vec<Event> as Archive>::Archived>,
    pub index: usize,
}

impl SelfIdentify for EventTapeMozak {
    fn set_self_identity(&mut self, id: ProgramIdentifier) { self.self_prog_id = id }

    fn get_self_identity(&self) -> ProgramIdentifier { self.self_prog_id }
}

impl EventEmit for EventTapeMozak {
    fn emit(&mut self, event: Event) {
        assert!(self.index < self.reader.unwrap().len());

        let zcd_event = &self.reader.unwrap()[self.index];
        let event_deserialized: Event = zcd_event.deserialize(&mut rkyv::Infallible).unwrap();

        assert_eq!(event, event_deserialized);

        assert_eq!(
            match event.operation {
                CanonicalEventType::Create
                | CanonicalEventType::Delete
                | CanonicalEventType::Write => event.object.constraint_owner,
                _ => self.self_prog_id,
            },
            self.self_prog_id
        );

        self.index += 1;
    }
}
