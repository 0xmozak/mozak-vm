use std::cell::UnsafeCell;
use std::ptr::slice_from_raw_parts;
use std::slice::from_raw_parts;

use mozak_system::system::syscall_halt;
use once_cell::unsync::Lazy;
use rkyv::ser::serializers::{AllocScratch, CompositeSerializer, HeapScratch};
use rkyv::{Archive, Deserialize, Serialize};

use crate::coretypes::{
    ArchivedCPCMessage, CPCMessage, Event, Poseidon2HashType, ProgramIdentifier, RawMessage,
};

pub type RkyvSerializer = rkyv::ser::serializers::AlignedSerializer<rkyv::AlignedVec>;
pub type RkyvScratch = rkyv::ser::serializers::FallbackScratch<HeapScratch<256>, AllocScratch>;
pub type RkyvShared = rkyv::ser::serializers::SharedSerializeMap;

pub trait RkyvSerializable =
    rkyv::Serialize<CompositeSerializer<RkyvSerializer, RkyvScratch, RkyvShared>>;
pub trait CallArgument = Sized + RkyvSerializable;
pub trait CallReturn = Sized + Clone + Default + RkyvSerializable;

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

// #[cfg(target_os = "zkvm")]
// extern "C" {
//     static _mozak_public_io_tape: usize;
//     // static _mozak_public_io_tape_len: usize;
//     static _mozak_private_io_tape: usize;
//     // static _mozak_private_io_tape_len: usize;
//     static _mozak_call_tape: u32;
//     // static _mozak_call_tape_len: usize;
//     static _mozak_event_tape: usize;
//     // static _mozak_event_tape_len: usize;
// }

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

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Copy, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct ProgramIdentifier2 {
    /// ProgramRomHash defines the hash of the text section of the
    /// static ELF program concerned
    pub program_rom_hash: Poseidon2HashType,

    /// MemoryInitHash defines the hash of the static memory initialization
    /// regions of the static ELF program concerned
    pub memory_init_hash: Poseidon2HashType,

    /// Entry point of the program
    pub entry_point: u32,
}

static mut SYSTEM_TAPES: Lazy<SystemTapes> = Lazy::new(|| {
    #[cfg(target_os = "zkvm")]
    {
        // These values should be derived from linker script and reserved memory
        // somewhere
        let call_tape_start: u32 = 0x40000000;

        // This runner should inject somewhere deterministic in memory and has to be
        // exact HARDCODED HERE: for token example, fix later
        let call_tape_len_dynamic = 244;
        type PreSerializationType = Vec<CPCMessage>;

        let archived = unsafe {
            let reserved_mem_slice =
                &*slice_from_raw_parts::<u8>(call_tape_start as *const u8, call_tape_len_dynamic);
            rkyv::archived_root::<PreSerializationType>(reserved_mem_slice)
        };

        let calls: PreSerializationType = archived.deserialize(&mut rkyv::Infallible).unwrap();

        // HARDCODED HERE: for token example, fix later
        assert!(calls.len() == 2);

        SystemTapes {
            call_tape: CallTape {
                writer: calls,
                ..Default::default()
            },
            ..SystemTapes::default()
        }
    }

    #[cfg(not(target_os = "zkvm"))]
    {
        // let calls = vec![CPCMessage::default(), CPCMessage::default()];
        SystemTapes::default()
    }
});

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct RawTape {
    start: u32,
    len: u32,
}

impl RawTape {
    pub fn new() -> Self { Self { start: 0, len: 0 } }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct CallTape {
    #[cfg(target_os = "zkvm")]
    self_prog_id: ProgramIdentifier,
    pub writer: Vec<CPCMessage>,
    #[cfg(target_os = "zkvm")]
    index: u32,
}

impl CallTape {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "zkvm")]
            self_prog_id: ProgramIdentifier::default(),
            writer: Vec::new(),
            #[cfg(target_os = "zkvm")]
            index: 0,
        }
    }

    #[cfg(target_os = "zkvm")]
    pub(crate) fn set_self_prog_id(&mut self, id: ProgramIdentifier) { self.self_prog_id = id; }

    pub fn from_mailbox<'a>(&'a mut self) -> Option<(&'a CPCMessage, usize)> {
        #[cfg(target_os = "zkvm")]
        {
            if self.index as usize >= self.writer.len() {
                return None;
            }

            self.index += 1;

            return Some((
                &self.writer[(self.index - 1) as usize],
                (self.index - 1) as usize,
            ));
        }

        #[cfg(not(target_os = "zkvm"))]
        {
            // TODO(bing): implement native from_mailbox()
            return None;
        }
    }

    pub fn to_mailbox<A, R>(
        &mut self,
        caller_prog: ProgramIdentifier,
        callee_prog: ProgramIdentifier,
        call_args: A,
        dispatch_native: impl Fn(A) -> R,
        _dispatch_zkvm: impl Fn() -> R,
    ) -> R
    where
        A: CallArgument,
        R: CallReturn, {
        #[cfg(not(target_os = "zkvm"))]
        {
            let msg = CPCMessage {
                caller_prog,
                callee_prog,
                args: rkyv::to_bytes::<_, 256>(&call_args).unwrap().into(),
                ..CPCMessage::default()
            };

            self.writer.push(msg);
            let inserted_idx = self.writer.len() - 1;

            let retval = dispatch_native(call_args);

            self.writer[inserted_idx].ret = rkyv::to_bytes::<_, 256>(&retval).unwrap().into();

            println!(
                "[CALL ] ResolvedAdd: {:#?}",
                self.writer[inserted_idx].clone()
            );

            retval
        }
        #[cfg(target_os = "zkvm")]
        {
            R::default()
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
    writer: Vec<EventTapeSingle>,
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct EventTapeSingle {
    id: ProgramIdentifier,
    contents: Vec<Event>,
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

    pub fn emit_event(&mut self, id: ProgramIdentifier, event: Event) {
        #[cfg(target_os = "zkvm")]
        {}
        #[cfg(not(target_os = "zkvm"))]
        {
            println!("[EVENT] Add: {:#?}", event);
            // TODO: Sad code, fix later
            for single_tape in self.writer.iter_mut() {
                if single_tape.id == id {
                    single_tape.contents.push(event);
                    return;
                }
            }
            self.writer.push(EventTapeSingle {
                id,
                contents: vec![event],
            });
            // unsafe {
            //     self.writer.push(event);
            // }
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
    let dbg_bytes = &format!("{:#?}", tape_clone).into_bytes();
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
pub fn event_emit(id: ProgramIdentifier, event: Event) {
    unsafe { SYSTEM_TAPES.event_tape.emit_event(id, event) }
}

/// Receive one message from mailbox targetted to us and its index
/// "consume" such message. Subsequent reads will never
/// return the same message. Panics on call-tape non-abidance.
pub fn call_receive<'a>() -> Option<(&'a CPCMessage, usize)> {
    // unsafe { SYSTEM_TAPES.call_tape.from_mailbox() }
    unsafe { SYSTEM_TAPES.call_tape.from_mailbox() }
}

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
pub fn call_send<A, R>(
    caller_prog: ProgramIdentifier,
    callee_prog: ProgramIdentifier,
    call_args: A,
    dispatch_native: impl Fn(A) -> R,
    dispatch_zkvm: impl Fn() -> R,
) -> R
where
    A: CallArgument,
    R: CallReturn, {
    unsafe {
        SYSTEM_TAPES.call_tape.to_mailbox(
            caller_prog,
            callee_prog,
            call_args,
            dispatch_native,
            dispatch_zkvm,
        )
    }
}

/// Get raw pointer to access iotape (unsafe) without copy into
/// buffer. Subsequent calls will provide pointers `num` away
/// (consumed) from pointer provided in this call for best
/// effort safety. `io_read` and `io_read_into` would also affect
/// subsequent returns.
/// Unsafe return values, use wisely!!
pub fn io_raw_read(_from: IOTape, _num: usize) -> *const u8 { unimplemented!() }

/// Get a buffer filled with num elements from choice of IOTape
/// in process "consuming" such bytes.
pub fn io_read(_from: IOTape, _num: usize) -> Vec<u8> { unimplemented!() }

/// Fills a provided buffer with num elements from choice of IOTape
/// in process "consuming" such bytes.
pub fn io_read_into(_from: IOTape, _buf: &mut [u8]) { unimplemented!() }
