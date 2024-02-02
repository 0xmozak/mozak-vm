#[cfg(not(target_os = "zkvm"))]
use std::cell::RefCell;
/// Unsafe code stays here and never leaves this file!!
use std::cell::UnsafeCell;

use once_cell::unsync::Lazy;
use rkyv::{Archive, Deserialize, Serialize};

use crate::coretypes::{CPCMessage, ContextVariable, Event, ProgramIdentifier};

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
    static _mozak_tapes_public_start: usize;
    static _mozak_tapes_public_len: usize;
    static _mozak_tapes_private_start: usize;
    static _mozak_tapes_private_len: usize;
    static _mozak_tapes_call_start: usize;
    static _mozak_tapes_call_len: usize;
    static _mozak_tapes_events_start: usize;
    static _mozak_tapes_events_len: usize;

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

    pub fn set_self_prog_id(&mut self, id: ProgramIdentifier) {
        self.call_tape.set_self_prog_id(id);
        self.event_tape.emit_event(Event::ReadContextVariable(
            ContextVariable::SelfProgramIdentifier(id),
        ));
    }
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
    // start: usize,
    // len: usize,
    // offset: UnsafeCell<usize>,
}

impl CallTape {
    pub fn new() -> Self {
        Self {
            self_prog_id: ProgramIdentifier::default(),
            // start: 0,
            // len: 0,
            // offset: UnsafeCell::new(0),
        }
    }

    pub(crate) fn set_self_prog_id(&mut self, id: ProgramIdentifier) { self.self_prog_id = id; }

    pub fn from_mailbox(&self) {}

    pub fn to_mailbox(&self, message: &CPCMessage) {}
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
    writer: Vec<Event>, // RefCell<Vec<Event<'static>>>
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
            writer: Vec::new(), //RefCell::new(Vec::new())
        }
    }

    pub fn emit_event(&mut self, event: Event) {
        #[cfg(target_os = "zkvm")]
        {}
        #[cfg(not(target_os = "zkvm"))]
        {
            println!("[EVENT] Add: {:?}", event);
            unsafe {
                self.writer.push(event);
            }
            // self.writer.borrow_mut().push(event);
        }
    }
}

pub fn emit_event(event: Event) { unsafe { SYSTEM_TAPES.event_tape.emit_event(event) } }

#[cfg(not(target_os = "zkvm"))]
pub fn dump_tapes(file_template: String) {
    fn write_to_file(file_path: &String, content: &[u8]) {
        use std::io::Write;
        let path = std::path::Path::new(file_path.as_str());
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
    }

    let dbg_filename = file_template.clone() + ".tape_debug";
    let dbg_bytes = unsafe { &format!("{:#?}", SYSTEM_TAPES).into_bytes() };
    println!("[TPDMP] Debug  dump: {:?}", dbg_filename);
    write_to_file(&dbg_filename, dbg_bytes);

    let bin_filename = file_template + ".tape_bin";
    let bin_bytes = unsafe {
        let clone = SYSTEM_TAPES.clone();
        // let extracted_value =
        //     once_cell::unsync::Lazy::<SystemTapes>::into_value(clone).unwrap();

        rkyv::to_bytes::<_, 256>(&*(std::ptr::addr_of!(clone)))
            .unwrap()

        // write_to_file(&bin_filename, bin_bytes.as_slice());
    };
    println!("[TPDMP] Binary dump: {:?}", bin_filename);
    write_to_file(&bin_filename, bin_bytes.as_slice());
}
