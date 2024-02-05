#[cfg(not(target_os = "zkvm"))]
use std::cell::RefCell;

/// Unsafe code stays here and never leaves this file!!
// use std::cell::UnsafeCell;
use once_cell::unsync::Lazy;
use rkyv::de::deserializers::SharedDeserializeMap;
use rkyv::ser::serializers::{
    AlignedSerializer, AllocScratch, CompositeSerializer, FallbackScratch, HeapScratch,
};
use rkyv::{AlignedVec, Archive, Deserialize, Serialize};

use crate::coretypes::{CPCMessage, Event, ProgramIdentifier};

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct SystemTapes {
    pub private_tape: RawTape,
    pub public_tape: RawTape,
    pub call_tape: CallTape,
    pub event_tape: EventTape,
}

#[cfg(target_os = "zkvm")]
extern "C" {
    static _mozak_tapes_start: usize;
    static _mozak_tapes_len: usize;

    // static _mozak_tapes_public_start: usize;
    // static _mozak_tapes_public_len: usize;
    // static _mozak_tapes_private_start: usize;
    // static _mozak_tapes_private_len: usize;
    // static _mozak_tapes_call_start: usize;
    // static _mozak_tapes_call_len: usize;
    // static _mozak_tapes_events_start: usize;
    // static _mozak_tapes_events_len: usize;

}

#[allow(dead_code)]
impl SystemTapes {
    fn new() -> Self {
        Self {
            private_tape: RawTape::new(),
            public_tape: RawTape::new(),
            call_tape: CallTape::new(),
            event_tape: EventTape::new(),
        }
    }

    // pub fn read_self_prog_id(&mut self, id: ProgramIdentifier) {
    //     self.call_tape.set_self_prog_id(id);
    //     self.event_tape.emit_event(Event::ReadContextVariable(
    //         ContextVariable::SelfProgramIdentifier(id),
    //     ));
    // }
}

static mut SYSTEM_TAPES: Lazy<SystemTapes> = Lazy::new(|| SystemTapes::new());

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct RawTape {
    // start: usize,
    // len: usize,
    // offset: UnsafeCell<usize>,
}

impl RawTape {
    pub fn new() -> Self {
        Self {
            // start: 0,
            // len: 0,
            // offset: UnsafeCell::new(0),
        }
    }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct CallTape {
    self_prog_id: ProgramIdentifier,
    #[cfg(not(target_os = "zkvm"))]
    writer: Vec<CPCMessage>,
}

impl CallTape {
    pub fn new() -> Self {
        Self {
            self_prog_id: ProgramIdentifier::default(),
            #[cfg(not(target_os = "zkvm"))]
            writer: Vec::new(),
        }
    }

    pub(crate) fn set_self_prog_id(&mut self, id: ProgramIdentifier) { self.self_prog_id = id; }

    pub fn from_mailbox(&self) {}

    pub fn to_mailbox<A, R>(
        &mut self,
        caller_prog: ProgramIdentifier,
        callee_prog: ProgramIdentifier,
        callee_fnid: u8,
        calldata: A,
        expected_return: R,
    ) where
        A: Sized
            + rkyv::Serialize<
                CompositeSerializer<
                    AlignedSerializer<AlignedVec>,
                    FallbackScratch<HeapScratch<256>, AllocScratch>,
                    SharedDeserializeMap,
                >,
            >,
        R: Sized
            + Clone
            + rkyv::Serialize<
                CompositeSerializer<
                    AlignedSerializer<AlignedVec>,
                    FallbackScratch<HeapScratch<256>, AllocScratch>,
                    SharedDeserializeMap,
                >,
            >, {
        #[cfg(not(target_os = "zkvm"))]
        {
            let args = unsafe { rkyv::to_bytes::<_, 256>(&calldata).unwrap() };
            let ret = unsafe { rkyv::to_bytes::<_, 256>(&expected_return).unwrap() };
            let msg = CPCMessage {
                caller_prog,
                callee_prog,
                callee_fnid,
                args: args.into(),
                ret: ret.into(),
            };

            println!("[CALL ] Add: {:#?}", msg);

            unsafe {
                self.writer.push(msg);
            }
        }
    }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct EventTape {
    // #[cfg(target_os = "zkvm")]
    // start: usize,
    // #[cfg(target_os = "zkvm")]
    // len: usize,
    // #[cfg(target_os = "zkvm")]
    // // offset: UnsafeCell<usize>,
    #[cfg(not(target_os = "zkvm"))]
    writer: Vec<Event>,
}

impl EventTape {
    pub fn new() -> Self {
        Self {
            // #[cfg(target_os = "zkvm")]
            // start: 0,
            // #[cfg(target_os = "zkvm")]
            // len: 0,
            // #[cfg(target_os = "zkvm")]
            // offset: UnsafeCell::new(0),
            #[cfg(not(target_os = "zkvm"))]
            writer: Vec::new(),
        }
    }

    pub fn emit_event(&mut self, event: Event) {
        #[cfg(target_os = "zkvm")]
        {}
        #[cfg(not(target_os = "zkvm"))]
        {
            println!("[EVENT] Add: {:#?}", event);
            unsafe {
                self.writer.push(event);
            }
            // self.writer.borrow_mut().push(event);
        }
    }
}

#[cfg(not(target_os = "zkvm"))]
pub fn dump_tapes(file_template: String) {
    use std::ptr::addr_of;
    fn write_to_file(file_path: &String, content: &[u8]) {
        use std::io::Write;
        let path = std::path::Path::new(file_path.as_str());
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
    }

    let tape_clone = unsafe { SYSTEM_TAPES.clone() }; // .clone() removes `Lazy{}`

    let dbg_filename = file_template.clone() + ".tape_debug";
    let dbg_bytes = unsafe { &format!("{:#?}", tape_clone).into_bytes() };
    println!("[TPDMP] Debug  dump: {:?}", dbg_filename);
    write_to_file(&dbg_filename, dbg_bytes);

    let bin_filename = file_template + ".tape_bin";
    let bin_bytes = unsafe { rkyv::to_bytes::<_, 256>(&*(addr_of!(tape_clone))).unwrap() };
    println!("[TPDMP] Binary dump: {:?}", bin_filename);
    write_to_file(&bin_filename, bin_bytes.as_slice());
}

/// ---- SDK accessible methods ---
pub enum IOTape {
    Private,
    Public,
}

/// Emit an event from mozak_vm to provide receipts of
/// `reads` and state updates including `create` and `delete`.
/// Panics on event-tape non-abidance.
pub fn event_emit(event: Event) { unsafe { SYSTEM_TAPES.event_tape.emit_event(event) } }

/// Receive one message from mailbox targetted to us and its index
/// "consume" such message. Subsequent reads will never
/// return the same message. Panics on call-tape non-abidance.
pub fn mailbox_receive() -> Option<(CPCMessage, usize)> { unimplemented!() }

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
pub fn mailbox_send<A, R>(
    caller_prog: ProgramIdentifier,
    callee_prog: ProgramIdentifier,
    callee_fnid: u8,
    calldata: A,
    expected_return: R,
) -> R
where
    A: Sized
        + rkyv::Serialize<
            CompositeSerializer<
                AlignedSerializer<AlignedVec>,
                FallbackScratch<HeapScratch<256>, AllocScratch>,
                SharedDeserializeMap,
            >,
        >,
    R: Sized
        + Clone
        + rkyv::Serialize<
            CompositeSerializer<
                AlignedSerializer<AlignedVec>,
                FallbackScratch<HeapScratch<256>, AllocScratch>,
                SharedDeserializeMap,
            >,
        >, {
    unsafe {
        SYSTEM_TAPES.call_tape.to_mailbox(
            caller_prog,
            callee_prog,
            callee_fnid,
            calldata,
            expected_return.clone(),
        )
    }
    expected_return
}

/// Get raw pointer to access iotape (unsafe) without copy into
/// buffer. Subsequent calls will provide pointers `num` away
/// (consumed) from pointer provided in this call for best
/// effort safety. `io_read` and `io_read_into` would also affect
/// subsequent returns.
/// Unsafe return values, use wisely!!
pub fn io_raw_read(from: IOTape, num: usize) -> *const u8 { unimplemented!() }

/// Get a buffer filled with num elements from choice of IOTape
/// in process "consuming" such bytes.
pub fn io_read(from: IOTape, num: usize) -> Vec<u8> { unimplemented!() }

/// Fills a provided buffer with num elements from choice of IOTape
/// in process "consuming" such bytes.
pub fn io_read_into(from: IOTape, buf: &mut [u8]) { unimplemented!() }
