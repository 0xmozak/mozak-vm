use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::common::traits::{EventEmit, SelfIdentify};
use crate::common::types::{Event, ProgramIdentifier};
use crate::native::helpers::IdentityStack;

/// Represents the `EventTape` under native execution
#[derive(Default, Clone)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct EventTape {
    #[serde(skip)]
    pub(crate) identity_stack: Rc<RefCell<IdentityStack>>,
    #[serde(rename = "individual_event_tapes")]
    pub(crate) writer: HashMap<ProgramIdentifier, Vec<Event>>,
}

impl std::fmt::Debug for EventTape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.writer.fmt(f) }
}

impl SelfIdentify for EventTape {
    fn set_self_identity(&mut self, _id: ProgramIdentifier) { unimplemented!() }

    fn get_self_identity(&self) -> ProgramIdentifier { self.identity_stack.borrow().top_identity() }
}

impl EventEmit for EventTape {
    fn emit(&mut self, event: Event) {
        assert_ne!(self.get_self_identity(), ProgramIdentifier::default());
        self.writer
            .entry(self.get_self_identity())
            .and_modify(|x| x.push(event.clone()))
            .or_insert(vec![event]);
    }
}
