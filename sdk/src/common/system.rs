#[cfg(not(target_os = "mozakvm"))]
use core::cell::RefCell;
#[cfg(not(target_os = "mozakvm"))]
use std::rc::Rc;

use once_cell::unsync::Lazy;
#[cfg(target_os = "mozakvm")]
use rkyv::rancor::{Panic, Strategy};
#[cfg(target_os = "mozakvm")]
use rkyv::Deserialize;

use super::types::{
    CallTapeType, EventTapeType, PrivateInputTapeType, PublicInputTapeType, SystemTape,
};
#[cfg(target_os = "mozakvm")]
use super::types::{CrossProgramCall, ProgramIdentifier};
#[cfg(target_os = "mozakvm")]
use crate::core::ecall::call_tape_read;

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
        let common_identity_stack = Rc::from(RefCell::new(
            crate::native::helpers::IdentityStack::default(),
        ));
        SystemTape {
            private_input_tape: PrivateInputTapeType {
                identity_stack: common_identity_stack.clone(),
                ..PrivateInputTapeType::default()
            },
            public_input_tape: PublicInputTapeType {
                identity_stack: common_identity_stack.clone(),
                ..PublicInputTapeType::default()
            },
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
        let mut buf = [0; 1024];
        call_tape_read(buf.as_mut_ptr(), 1024);

        let messages_raw = unsafe { rkyv::access_unchecked::<Vec<CrossProgramCall>>(&buf) };

        let messages = <<Vec<CrossProgramCall> as rkyv::Archive>::Archived as Deserialize<
            Vec<CrossProgramCall>,
            Strategy<(), Panic>,
        >>::deserialize(messages_raw, Strategy::wrap(&mut ()))
        .unwrap();

        SystemTape {
            private_input_tape: PrivateInputTapeType::default(),
            public_input_tape: PublicInputTapeType::default(),
            call_tape: CallTapeType {
                cast_list: vec![],
                self_prog_id: ProgramIdentifier::default(),
                reader: Some(messages),
                index: 0,
            },

            event_tape: EventTapeType::default(),
        }
    }
});

#[cfg(target_os = "mozakvm")]
#[allow(dead_code)]
pub fn ensure_clean_shutdown() {
    // Ensure we have read the whole tape
    unsafe {
        // Should have read the full call tape
        assert!(
            SYSTEM_TAPE.call_tape.index == SYSTEM_TAPE.call_tape.reader.as_ref().unwrap().len()
        );

        // Should have read the full event tape
        assert!(SYSTEM_TAPE.event_tape.index == SYSTEM_TAPE.event_tape.reader.unwrap().len());
    }
}
