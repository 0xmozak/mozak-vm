use std::cell::RefCell;
use std::collections::HashMap;

use crate::native_helpers::IdentityStack;
use crate::traits::{EventEmit, SelfIdentify};
use crate::types::{Event, ProgramIdentifier};

/// Represents the `EventTape` under native execution
#[derive(Default)]
pub struct EventTapeNative {
    pub writer: HashMap<ProgramIdentifier, Vec<Event>>,
    pub identity_stack: RefCell<IdentityStack>,
}

impl SelfIdentify for EventTapeNative {
    fn set_self_identity(&mut self, _id: ProgramIdentifier) { unimplemented!() }

    fn get_self_identity(&self) -> ProgramIdentifier { unimplemented!() }
}

impl EventEmit for EventTapeNative {
    fn emit(&mut self, event: Event) {
        assert_ne!(self.get_self_identity(), ProgramIdentifier::default());
        self.writer
            .entry(self.identity_stack.borrow().top_identity())
            .and_modify(|x| x.push(event.clone()))
            .or_insert(vec![event]);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_event_emit() {
        type A = u8;
        type B = u16;

        let mut calltape = EventTapeNative::default();

        let resolver = |val: A| -> B { (val + 1) as B };

        let response = calltape.send(test_pid_generator(1), 1 as A, resolver);
        assert_eq!(response, 2);
        assert_eq!(calltape.writer.len(), 1);
        assert_eq!(calltape.writer[0].caller_prog, ProgramIdentifier::default());
        assert_eq!(calltape.writer[0].callee_prog, test_pid_generator(1));
    }
}
