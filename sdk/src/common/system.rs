use once_cell::unsync::Lazy;
#[cfg(target_os = "mozakvm")]
use {
    crate::common::merkle::merkleize,
    crate::common::types::{CanonicalOrderedTemporalHints, CrossProgramCall, Poseidon2Hash},
    crate::core::constants::DIGEST_BYTES,
    crate::core::ecall::{
        call_tape_read, event_tape_read, ioread_private, ioread_public, self_prog_id_tape_read,
    },
    rkyv::rancor::{Panic, Strategy},
    std::collections::BTreeSet,
};
#[cfg(not(target_os = "mozakvm"))]
use {core::cell::RefCell, std::rc::Rc};

use crate::common::traits::{
    ArchivedCallArgument, ArchivedCallReturn, Call, CallArgument, CallReturn, EventEmit,
};
use crate::common::types::{
    CallTapeType, Event, EventTapeType, PrivateInputTapeType, ProgramIdentifier,
    PublicInputTapeType, SystemTape,
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
        let mut self_prog_id_bytes = [0; DIGEST_BYTES];
        self_prog_id_tape_read(&mut self_prog_id_bytes);
        let self_prog_id = ProgramIdentifier(Poseidon2Hash::from(self_prog_id_bytes));

        let call_tape = populate_call_tape(self_prog_id);
        let event_tape = populate_event_tape(self_prog_id);

        let mut size_hint_bytes = [0; 4];

        ioread_public(&mut size_hint_bytes);
        let size_hint: usize = u32::from_le_bytes(size_hint_bytes).try_into().unwrap();
        let public_input_tape = PublicInputTapeType::with_size_hint(size_hint);

        ioread_private(&mut size_hint_bytes);
        let size_hint: usize = u32::from_le_bytes(size_hint_bytes).try_into().unwrap();
        let private_input_tape = PrivateInputTapeType::with_size_hint(size_hint);

        SystemTape {
            private_input_tape,
            public_input_tape,
            call_tape,
            event_tape,
        }
    }
});

#[cfg(target_os = "mozakvm")]
/// Populates a `MozakVM` [`CallTapeType`] via ECALLs.
///
/// At this point, the [`CrossProgramCall`] messages are still rkyv-serialized,
/// and must be deserialized at the point of consumption. Only the `callee`s are
/// deserialized for persistence of the `cast_list`.
///
/// This function deliberately leaks the backing buffer for the storage of the
/// returned call tape.
fn populate_call_tape(self_prog_id: ProgramIdentifier) -> CallTapeType {
    let mut len_bytes = [0; 4];
    call_tape_read(&mut len_bytes);
    let len: usize = u32::from_le_bytes(len_bytes).try_into().unwrap();
    let buf: &'static mut Vec<u8> = Box::leak(Box::new(vec![0; len]));
    call_tape_read(buf);

    let archived_cpc_messages = rkyv::access::<Vec<CrossProgramCall>, Panic>(buf).unwrap();

    let cast_list: Vec<ProgramIdentifier> = archived_cpc_messages
        .iter()
        .map(|m| {
            m.callee
                .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                .unwrap()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    CallTapeType {
        cast_list,
        self_prog_id,
        reader: Some(archived_cpc_messages),
        index: 0,
    }
}

#[cfg(target_os = "mozakvm")]
/// Populates a `MozakVM` [`EventTapeType`] via ECALLs.
///
/// At this point, the vector of [`CanonicalOrderedTemporalHints`] are still
/// rkyv-serialized, and must be deserialized at the point of consumption.
///
/// This function deliberately leaks the backing buffer for the storage of the
/// returned event tape.
fn populate_event_tape(self_prog_id: ProgramIdentifier) -> EventTapeType {
    let mut len_bytes = [0; 4];
    event_tape_read(&mut len_bytes);

    let len: usize = u32::from_le_bytes(len_bytes).try_into().unwrap();
    let buf: &'static mut Vec<u8> = Box::leak(Box::new(vec![0; len]));
    event_tape_read(buf);

    let canonical_ordered_temporal_hints =
        rkyv::access::<Vec<CanonicalOrderedTemporalHints>, Panic>(buf).unwrap();
    EventTapeType {
        self_prog_id,
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

/// Receive one message from mailbox targetted to us and its index
/// "consume" such message. Subsequent reads will never
/// return the same message. Panics on call-tape non-abidance.
#[must_use]
pub fn call_receive<A, R>() -> Option<(ProgramIdentifier, A, R)>
where
    A: CallArgument + PartialEq,
    R: CallReturn,
    <A as rkyv::Archive>::Archived: ArchivedCallArgument<A>,
    <R as rkyv::Archive>::Archived: ArchivedCallReturn<R>, {
    unsafe { SYSTEM_TAPE.call_tape.receive() }
}

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
#[allow(clippy::similar_names)]
pub fn call_send<A, R>(
    recipient_program: ProgramIdentifier,
    argument: A,
    resolver: impl Fn(A) -> R,
) -> R
where
    A: CallArgument + PartialEq,
    R: CallReturn,
    <A as rkyv::Archive>::Archived: ArchivedCallArgument<A>,
    <R as rkyv::Archive>::Archived: ArchivedCallReturn<R>, {
    unsafe {
        SYSTEM_TAPE
            .call_tape
            .send(recipient_program, argument, resolver)
    }
}

#[cfg(target_os = "mozakvm")]
#[allow(dead_code)]
pub fn ensure_clean_shutdown() {
    // Ensure we have read the whole tape

    unsafe {}
}
