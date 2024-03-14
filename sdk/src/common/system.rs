use once_cell::unsync::Lazy;
#[cfg(target_os = "mozakvm")]
use rkyv::Deserialize;

use super::types::SystemTape;
#[cfg(target_os = "mozakvm")]
use crate::common::types::{CPCMessage, CallTapeType, ProgramIdentifier};
#[cfg(target_os = "mozakvm")]
use crate::mozakvm::helpers::{
    archived_repr, get_rkyv_archived, get_rkyv_deserialized, get_self_prog_id,
};
#[cfg(target_os = "mozakvm")]
use crate::mozakvm::linker_symbols::{mozak_call_tape, mozak_cast_list};

/// `SYSTEM_TAPES` is a global singleton for interacting with
/// all the `IO-Tapes`, `CallTape` and the `EventTape` both in
/// native as well as mozakvm environment.
#[allow(dead_code)]
static mut SYSTEM_TAPES: Lazy<SystemTape> = Lazy::new(|| {
    // The following is initialization of `SYSTEM_TAPES` in native.
    // In most cases, when run in native, `SYSTEM_TAPES` is used to
    // generate and fill the elements found within `CallTape`,
    // `EventTape` etc. As such, an empty `SystemTapes` works here.
    #[cfg(not(target_os = "mozakvm"))]
    {
        SystemTape::default()
    }

    // On the other hand, when `SYSTEM_TAPES` is used in mozakvm,
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
                cast_list: get_rkyv_deserialized!(Vec<ProgramIdentifier>, mozak_cast_list),
                reader: Some(get_rkyv_archived!(Vec<CPCMessage>, mozak_call_tape)),
                index: 0,
            },
        }
    }
});
