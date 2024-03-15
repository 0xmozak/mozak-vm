use once_cell::unsync::Lazy;
#[cfg(target_os = "mozakvm")]
use rkyv::Deserialize;

use super::types::SystemTape;
#[cfg(target_os = "mozakvm")]
use crate::common::types::{CPCMessage, CallTapeType, Event, EventTapeType, ProgramIdentifier};
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
static mut SYSTEM_TAPE: Lazy<SystemTape> = Lazy::new(|| {
    // The following is initialization of `SYSTEM_TAPE` in native.
    // In most cases, when run in native, `SYSTEM_TAPE` is used to
    // generate and fill the elements found within `CallTape`,
    // `EventTape` etc. As such, an empty `SystemTapes` works here.
    #[cfg(not(target_os = "mozakvm"))]
    {
        SystemTape::default()
    }

    // On the other hand, when `SYSTEM_TAPE` is used in mozakvm,
    // It is used to "validate" the underlying tapes such as
    // `CallTape` and `EventTape`. When run in VM, the loader
    // pre-populates specific memory locations with a ZCD representation
    // of what we need. As such, we need to read up with those
    // pre-populated data elements
    #[cfg(target_os = "mozakvm")]
    {
        SystemTape {
            call_tape: CallTapeType {
                self_prog_id: get_self_prog_id(),
                cast_list: get_rkyv_deserialized!(Vec<ProgramIdentifier>, _mozak_cast_list),
                reader: Some(get_rkyv_archived!(Vec<CPCMessage>, _mozak_call_tape)),
                index: 0,
            },
            event_tape: EventTapeType {
                self_prog_id: get_self_prog_id(),
                reader: Some(get_rkyv_archived!(Vec<Event>, _mozak_event_tape)),
                index: 0,
            },
        }
    }
});
