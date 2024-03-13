use once_cell::unsync::Lazy;

#[cfg(target_os = "mozakvm")]
use crate::mozakvm_helpers::{archived_repr, get_self_prog_id};
#[cfg(target_os = "mozakvm")]
use crate::mozakvm_linker_symbols::{mozak_call_tape, mozak_cast_list};
#[cfg(target_os = "mozakvm")]
use crate::types::{CPCMessage, ProgramIdentifier};

#[cfg(target_os = "mozakvm")]
type SystemTapeCallTapeType = crate::mozakvm_calltape::CallTapeMozakVM;
#[cfg(not(target_os = "mozakvm"))]
type SystemTapeCallTapeType = crate::native_calltape::CallTapeNative;

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct SystemTapes {
    // TODO: Add Public and Private IO Tape
    pub call_tape: SystemTapeCallTapeType,
    // pub event_tape: EventTape,
}

/// `SYSTEM_TAPES` is a global singleton for interacting with
/// all the `IO-Tapes`, `CallTape` and the `EventTape` both in
/// native as well as mozakvm environment.
#[allow(dead_code)]
static mut SYSTEM_TAPES: Lazy<SystemTapes> = Lazy::new(|| {
    // The following is initialization of `SYSTEM_TAPES` in native.
    // In most cases, when run in native, `SYSTEM_TAPES` is used to
    // generate and fill the elements found within `CallTape`,
    // `EventTape` etc. As such, an empty `SystemTapes` works here.
    #[cfg(not(target_os = "mozakvm"))]
    {
        SystemTapes::default()
    }

    // On the other hand, when `SYSTEM_TAPES` is used in mozakvm,
    // It is used to "validate" the underlying tapes such as
    // `CallTape` and `EventTape`. When run in VM, the loader
    // pre-populates specific memory locations with a ZCD representation
    // of what we need. As such, we need to read up with those
    // pre-populated data elements
    #[cfg(target_os = "mozakvm")]
    {
        // Firstly, get to know who we are!
        let _pid = get_self_prog_id();

        // Then, get archive access to elements in memory

        macro_rules! mem_begin {
            ($x:expr) => {
                #[allow(clippy::ptr_as_ptr)]
                {
                    unsafe { core::ptr::addr_of!($x) as *const u8 }
                }
            };
        }

        let _castlist_ar = archived_repr::<Vec<ProgramIdentifier>>(mem_begin!(mozak_cast_list));
        let _calltape_ar = archived_repr::<Vec<CPCMessage>>(mem_begin!(mozak_call_tape));
        // let evnttape_ar = archived_repr::<Vec<Event>>(mem_begin!(mozak_event_tape));

        SystemTapes::default()
    }
});
