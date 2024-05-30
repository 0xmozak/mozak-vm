use once_cell::unsync::Lazy;
use rkyv::rancor::{Panic, Strategy};
use rkyv::Deserialize;
#[cfg(target_os = "mozakvm")]
use {
    crate::common::merkle::merkleize,
    crate::common::types::{CanonicalOrderedTemporalHints, CrossProgramCall, Poseidon2Hash},
    // crate::core::constants::DIGEST_BYTES,
    crate::core::ecall::{
        call_tape_read,
        event_tape_read,
        // ioread_private, ioread_public, self_prog_id_tape_read,
    },
    core::ptr::slice_from_raw_parts,
    // std::collections::BTreeSet,
};
#[cfg(not(target_os = "mozakvm"))]
use {core::cell::RefCell, std::rc::Rc};

use crate::common::traits::{Call, EventEmit};
use crate::common::types::{
    CallTapeType, Event, EventTapeType, PrivateInputTapeType, PublicInputTapeType, RoleIdentifier,
    SystemTape,
};

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
            crate::native::identity::IdentityStack::default(),
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
        type _x = PrivateInputTapeType;
        type _y = PublicInputTapeType;
        // TODO: Fix
        // let mut self_prog_id_bytes = [0; DIGEST_BYTES];
        // self_prog_id_tape_read(self_prog_id_bytes.as_mut_ptr()); // Implement
        // self_role_id_tape_read let self_prog_id =
        // ProgramIdentifier(Poseidon2Hash::from(self_prog_id_bytes));

        // let call_tape = populate_call_tape(self_prog_id);
        // let event_tape = populate_event_tape(self_prog_id);

        // let mut size_hint_bytes = [0; 4];

        // ioread_public(size_hint_bytes.as_mut_ptr(), 4);
        // let size_hint: usize =
        // u32::from_le_bytes(size_hint_bytes).try_into().unwrap();
        // let public_input_tape = PublicInputTapeType::with_size_hint(size_hint);

        // ioread_private(size_hint_bytes.as_mut_ptr(), 4);
        // let size_hint: usize =
        // u32::from_le_bytes(size_hint_bytes).try_into().unwrap();
        // let private_input_tape = PrivateInputTapeType::with_size_hint(size_hint);

        // SystemTape {
        //     private_input_tape,
        //     public_input_tape,
        //     call_tape,
        //     event_tape,
        // }
        SystemTape::default()
    }
});

#[cfg(target_os = "mozakvm")]
#[allow(warnings)]
/// Populates a `MozakVM` [`CallTapeType`] via ECALLs.
///
/// At this point, the [`CrossProgramCall`] messages are still rkyv-serialized,
/// and must be deserialized at the point of consumption. Only the `callee`s are
/// deserialized for persistence of the `cast_list`.
fn populate_call_tape(self_role_id: RoleIdentifier) -> CallTapeType {
    let mut len_bytes = [0; 4];
    call_tape_read(len_bytes.as_mut_ptr(), 4);
    let len: usize = u32::from_le_bytes(len_bytes).try_into().unwrap();
    let mut buf = Vec::with_capacity(len);
    call_tape_read(buf.as_mut_ptr(), len);

    let archived_cpc_messages = unsafe {
        rkyv::access_unchecked::<Vec<CrossProgramCall>>(&*slice_from_raw_parts(buf.as_ptr(), len))
    };

    // let cast_list: Vec<RoleIdentifier> = archived_cpc_messages
    //     .iter()
    //     .map(|m| {
    //         m.callee
    //             .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
    //             .unwrap()
    //     })
    //     .collect::<BTreeSet<_>>()
    //     .into_iter()
    //     .collect();

    CallTapeType {
        cast_list: Vec::default(),
        self_role_id,
        reader: Some(archived_cpc_messages),
        index: 0,
    }
}

#[cfg(target_os = "mozakvm")]
#[allow(warnings)]
/// Populates a `MozakVM` [`EventTapeType`] via ECALLs.
///
/// At this point, the vector of [`CanonicalOrderedTemporalHints`] are still
/// rkyv-serialized, and must be deserialized at the point of consumption.
fn populate_event_tape(self_role_id: RoleIdentifier) -> EventTapeType {
    let mut len_bytes = [0; 4];
    event_tape_read(len_bytes.as_mut_ptr(), 4);
    let len: usize = u32::from_le_bytes(len_bytes).try_into().unwrap();
    let mut buf = Vec::with_capacity(len);
    event_tape_read(buf.as_mut_ptr(), len);

    let canonical_ordered_temporal_hints = unsafe {
        rkyv::access_unchecked::<Vec<CanonicalOrderedTemporalHints>>(&*slice_from_raw_parts(
            buf.as_ptr(),
            len,
        ))
    };

    EventTapeType {
        self_role_id,
        reader: Some(canonical_ordered_temporal_hints),
        seen: vec![false; canonical_ordered_temporal_hints.len()],
        index: 0,
    }
}

/// Emit an event from `mozak_vm` to provide receipts of
/// `reads` and state updates including `create` and `delete`.
/// Panics on event-tape non-abidance.
pub fn event_emit(event: Event) {
    unsafe {
        SYSTEM_TAPE.event_tape.emit(event);
    }
}

/// Gets a roleID determined fully by `(Prog, instance)` tuple. It is
/// guaranteed that any call wih same `(Prog, instance)` tuple during one
/// native context will always return the same `RoleIdentifier` within that
/// context. Useful when different programs need to call the same role.
#[cfg(not(target_os = "mozakvm"))]
pub fn get_deterministic_role_id(
    prog: crate::common::types::ProgramIdentifier,
    instance: String,
) -> RoleIdentifier {
    unsafe {
        SYSTEM_TAPE
            .call_tape
            .get_deterministic_role_id(prog, instance)
    }
}

/// Gets a fresh & unique roleID referencible only by the `RoleIdentifier`
#[cfg(not(target_os = "mozakvm"))]
pub fn get_unique_role_id(prog: crate::common::types::ProgramIdentifier) -> RoleIdentifier {
    unsafe { SYSTEM_TAPE.call_tape.get_unique_role_id(prog) }
}

/// Receive one message from mailbox targetted to us and its index
/// "consume" such message. Subsequent reads will never
/// return the same message. Panics on call-tape non-abidance.
#[must_use]
pub fn call_receive<A, R>() -> Option<(crate::common::types::RoleIdentifier, A, R)>
where
    A: crate::common::traits::CallArgument + PartialEq,
    R: crate::common::traits::CallReturn,
    <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
    <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
    unsafe { crate::common::system::SYSTEM_TAPE.call_tape.receive() }
}

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
#[allow(clippy::similar_names)]
pub fn call_send<A, R>(
    recipient: crate::common::types::RoleIdentifier,
    argument: A,
    resolver: impl Fn(A) -> R,
) -> R
where
    A: crate::common::traits::CallArgument + PartialEq,
    R: crate::common::traits::CallReturn,
    <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
    <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .send(recipient, argument, resolver)
    }
}

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
        assert!(
            SYSTEM_TAPE.event_tape.index == SYSTEM_TAPE.event_tape.reader.as_ref().unwrap().len()
        );

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
            cast_list
                .iter()
                .enumerate()
                .map(|(idx, x)| (idx as u64, x.0))
                .collect(),
        )
        .0;

        assert!(claimed_commitment_cl == calculated_commitment_cl);
    }
}
