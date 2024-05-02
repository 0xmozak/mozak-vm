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
use crate::common::types::{CanonicalOrderedTemporalHints, CrossProgramCall, ProgramIdentifier};
#[cfg(target_os = "mozakvm")]
use crate::common::{merkle::merkleize, types::Poseidon2Hash};
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
        let events = get_rkyv_archived!(Vec<CanonicalOrderedTemporalHints>, _mozak_event_tape);

        SystemTape {
            private_input_tape: PrivateInputTapeType::default(),
            public_input_tape: PublicInputTapeType::default(),
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

    use itertools::izip;
    unsafe {
        // Should have read the full call tape
        assert!(SYSTEM_TAPE.call_tape.index == SYSTEM_TAPE.call_tape.reader.unwrap().len());

        // Should have read the full event tape
        assert!(SYSTEM_TAPE.event_tape.index == SYSTEM_TAPE.event_tape.reader.unwrap().len());

        // Assert that event commitment tape has the same bytes
        // as Event Tape's actual commitment observable to us
        let mut claimed_commitment_ev: [u8; 32] = [0; 32];
        crate::core::ecall::events_commitment_tape_read(claimed_commitment_ev.as_mut_ptr());

        let canonical_event_temporal_hints: Vec<CanonicalOrderedTemporalHints> = SYSTEM_TAPE
            .event_tape
            .reader
            .unwrap()
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();

        let calculated_commitment_ev = merkleize(
            canonical_event_temporal_hints
                .iter()
                .map(|x| {
                    (
                        // May not be the best idea if
                        // `addr` > goldilock's prime, cc
                        // @Kapil
                        u64::from_le_bytes(x.0.address.inner()),
                        x.0.canonical_hash(),
                    )
                })
                .collect::<Vec<(u64, Poseidon2Hash)>>(),
        )
        .0;

        assert!(claimed_commitment_ev == calculated_commitment_ev);

        // Assert that castlist commitment tape has the same bytes
        // as CastList's actual commitment observable to us
        let mut claimed_commitment_cl: [u8; 32] = [0; 32];
        crate::core::ecall::cast_list_commitment_tape_read(claimed_commitment_cl.as_mut_ptr());

        let cast_list = &SYSTEM_TAPE.call_tape.cast_list;

        let calculated_commitment_cl = merkleize(
            izip!(0.., cast_list)
                .map(|(idx, x)| (idx, x.0))
                .collect(),
        )
        .0;

        assert!(claimed_commitment_cl == calculated_commitment_cl);
    }
}
