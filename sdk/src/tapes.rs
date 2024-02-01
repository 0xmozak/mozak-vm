/// Unsafe code stays here and never leaves this file!!

use std::{cell::UnsafeCell, ptr::slice_from_raw_parts};
use once_cell::unsync::Lazy;

use crate::coretypes::{CPCMessage, ProgramIdentifier};

// use lazy_static::lazy_static;
pub struct SystemTapes {
    pub private_tape: RawTape,
    pub public_tape: RawTape,
    pub call_tape: CallTape,
    pub event_tape: EventTape,
}

#[cfg(target_os = "zkvm")]
extern "C" {
    static _mozak_tapes_public_start:  usize;
    static _mozak_tapes_public_len:    usize;
    static _mozak_tapes_private_start: usize;
    static _mozak_tapes_private_len:   usize;
    static _mozak_tapes_call_start:    usize;
    static _mozak_tapes_call_len:      usize;
    static _mozak_tapes_events_start:  usize;
    static _mozak_tapes_events_len:    usize;

}

#[allow(dead_code)]
impl SystemTapes {
    fn new() ->Self {
        Self {
            private_tape: RawTape::new(),
            public_tape: RawTape::new(),
            call_tape: CallTape::new(),
            event_tape: EventTape::new(),
        }
    }

    pub fn set_self_prog_id(&mut self, id: ProgramIdentifier) {
        self.call_tape.set_self_prog_id(id);
    }
}

static mut SYSTEM_TAPES: Lazy<SystemTapes> = Lazy::new(|| {
    SystemTapes::new()
});

pub struct RawTape{
    start: usize,
    len: usize,
    offset: UnsafeCell<usize>,
}

impl RawTape {
    pub fn new() -> Self {
        Self {
            start: 0,
            len: 0,
            offset: UnsafeCell::new(0),
        }
    }
}

pub struct CallTape{
    self_prog_id: ProgramIdentifier,
    start: usize,
    len: usize,
    offset: UnsafeCell<usize>,
}

impl CallTape {
    pub fn new() -> Self {
        Self {
            self_prog_id: ProgramIdentifier::default(),
            start: 0,
            len: 0,
            offset: UnsafeCell::new(0),
        }
    }

    pub(crate) fn set_self_prog_id(&mut self, id: ProgramIdentifier) {
        self.self_prog_id = id;
    }

    pub fn from_mailbox(&self) {

    }

    pub fn to_mailbox(&self, message: &CPCMessage) {

    }
}

pub struct EventTape{
    start: usize,
    len: usize,
    offset: UnsafeCell<usize>,
}

impl EventTape {
    pub fn new() -> Self {
        Self {
            start: 0,
            len: 0,
            offset: UnsafeCell::new(0),
        }
    }

    pub fn emit_event(event: &[u8]) {

    }
}
