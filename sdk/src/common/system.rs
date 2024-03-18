#[cfg(not(target_os = "mozakvm"))]
use core::cell::RefCell;
#[cfg(not(target_os = "mozakvm"))]
use std::rc::Rc;

use once_cell::unsync::Lazy;
#[cfg(target_os = "mozakvm")]
use rkyv::Deserialize;

use super::types::{CallTapeType, EventTapeType, SystemTape};
#[cfg(target_os = "mozakvm")]
use crate::common::types::{CanonicalOrderedTemporalHints, CrossProgramCall, ProgramIdentifier};
#[cfg(target_os = "mozakvm")]
use crate::mozakvm::helpers::{
    archived_repr, get_rkyv_archived, get_rkyv_deserialized, get_self_prog_id,
};
#[cfg(target_os = "mozakvm")]
use crate::mozakvm::linker_symbols::{_mozak_call_tape, _mozak_cast_list, _mozak_event_tape};

/// `SYSTEM_TAPE` is a global singleton for interacting with
/// all the `IO-Tapes`, `CallTape` and the `EventTape` both in
/// native as well as mozakvm environment.
#[allow(dead_code)]
pub(crate) static mut SYSTEM_TAPE: Lazy<SystemTape> = Lazy::new(|| {
    // The following is initialization of `SYSTEM_TAPE` in native.
    // In most cases, when run in native, `SYSTEM_TAPE` is used to
    // generate and fill the elements found within `CallTape`,
    // `EventTape` etc. As such, an empty `SystemTapes` works here.
    #[cfg(not(target_os = "mozakvm"))]
    {
        let common_identity_stack =
            Rc::from(RefCell::new(crate::native::helpers::IdentityStack::default()));
        SystemTape {
            call_tape: CallTapeType {
                identity_stack: common_identity_stack.clone(),
                ..CallTapeType::default()
            },
            event_tape: EventTapeType {
                identity_stack: common_identity_stack,
                ..EventTapeType::default()
            },
        }
    }

    // On the other hand, when `SYSTEM_TAPE` is used in mozakvm,
    // It is used to "validate" the underlying tapes such as
    // `CallTape` and `EventTape`. When run in VM, the loader
    // pre-populates specific memory locations with a ZCD representation
    // of what we need. As such, we need to read up with those
    // pre-populated data elements
    #[cfg(target_os = "mozakvm")]
    {
        let events = get_rkyv_archived!(Vec<CanonicalOrderedTemporalHints>, _mozak_event_tape);

        SystemTape {
            call_tape: CallTapeType {
                self_prog_id: get_self_prog_id(),
                cast_list: get_rkyv_deserialized!(Vec<ProgramIdentifier>, _mozak_cast_list),
                reader: Some(get_rkyv_archived!(Vec<CrossProgramCall>, _mozak_call_tape)),
                index: 0,
            },
            event_tape: EventTapeType {
                self_prog_id: get_self_prog_id(),
                reader: Some(events),
                seen: vec![false; events.len()],
                index: 0,
            },
        }
    }
});

#[cfg(target_os = "mozakvm")]
#[allow(dead_code)]
pub fn ensure_clean_shutdown() {
    // Ensure we have read the whole tape
    unsafe {
        // Should have read the full call tape
        assert!(SYSTEM_TAPE.call_tape.index == SYSTEM_TAPE.call_tape.reader.unwrap().len());

        // Should have read the full event tape
        assert!(SYSTEM_TAPE.event_tape.index == SYSTEM_TAPE.event_tape.reader.unwrap().len());
    }
}
