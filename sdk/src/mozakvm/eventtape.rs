use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize};

use crate::common::traits::{EventEmit, SelfIdentify};
use crate::common::types::{
    CanonicalEvent, CanonicalOrderedTemporalHints, Event, ProgramIdentifier,
    SelfCallExtendedProgramIdentifier, SelfCallExtensionFlag,
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
    fn set_self_identity(&mut self, id: SelfCallExtendedProgramIdentifier) {
        self.self_prog_id = id.0;
    }

    // WARNING: returns from this function does not provide
    // the correct `SelfCallExtensionFlag` simply because event
    // tape doesn't need it for anything.
    fn get_self_identity(&self) -> SelfCallExtendedProgramIdentifier {
        SelfCallExtendedProgramIdentifier(self.self_prog_id, SelfCallExtensionFlag::default())
    }
}

impl EventEmit for EventTape {
    fn emit(&mut self, event: Event) {
        assert!(self.index < self.reader.unwrap().len());
        let generated_canonical_event = CanonicalEvent::from_event(self.self_prog_id, &event);

        let elem_idx: usize = self.reader.unwrap()[self.index]
            .1
            .to_native()
            .try_into()
            .unwrap();
        assert!(!self.seen[elem_idx]);
        self.seen[elem_idx] = true;

        let zcd_canonical_event = &self.reader.unwrap()[self.index].0;
        let canonical_event: CanonicalEvent = zcd_canonical_event
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();

        assert!(canonical_event == generated_canonical_event);
        self.index += 1;
    }
}
