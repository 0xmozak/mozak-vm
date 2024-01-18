// Implementation for various ecall functions.

use std::str::from_utf8;

use mozak_system::system::ecall;
use mozak_system::system::reg_abi::{REG_A0, REG_A1, REG_A2};
use plonky2::hash::hash_types::RichField;

use crate::state::{Aux, IoEntry, IoOpcode, State};

impl<F: RichField> State<F> {
    fn ecall_halt(self) -> (Aux<F>, Self) {
        // Note: we don't advance the program counter for 'halt'.
        // That is we treat 'halt' like an endless loop.
        (
            Aux {
                will_halt: true,
                ..Aux::default()
            },
            self.halt(),
        )
    }

    /// # Panics
    ///
    /// Panics if while executing `IO_READ`, I/O tape does not have sufficient
    /// bytes.
    fn ecall_io_read(self, op: IoOpcode) -> (Aux<F>, Self) {
        let buffer_start = self.get_register_value(REG_A1);
        let num_bytes_requsted = self.get_register_value(REG_A2);
        let (data, updated_self) = self.read_iobytes(num_bytes_requsted as usize, op);
        log::debug!(
            "ecall_io_read: 0x{:0x}, {:?}, {:?} ",
            buffer_start,
            num_bytes_requsted,
            data
        );
        (
            Aux {
                dst_val: u32::try_from(data.len()).expect("cannot fit data.len() into u32"),
                io: Some(IoEntry {
                    addr: buffer_start,
                    op,
                    data: data.clone(),
                }),
                ..Default::default()
            },
            data.iter()
                .enumerate()
                .fold(updated_self, |updated_self, (i, byte)| {
                    updated_self
                        .store_u8(
                            buffer_start
                                .wrapping_add(u32::try_from(i).expect("cannot fit i into u32")),
                            *byte,
                        )
                        .unwrap()
                })
                .bump_pc(),
        )
    }

    /// # Panics
    ///
    /// Panics if Vec<u8> to string conversion fails.
    fn ecall_panic(self) -> (Aux<F>, Self) {
        let msg_len = self.get_register_value(REG_A1);
        let msg_ptr = self.get_register_value(REG_A2);
        let mut msg_vec = vec![];
        for addr in msg_ptr..(msg_ptr + msg_len) {
            msg_vec.push(self.load_u8(addr));
        }
        panic!(
            "VM panicked with msg: {}",
            from_utf8(&msg_vec).expect("A valid utf8 VM panic message should be provided")
        );
    }

    /// Outputs the VM trace log at `clk`. Useful for debugging.
    /// # Panics
    ///
    /// Panics if Vec<u8> to string conversion fails.
    fn ecall_trace_log(self) -> (Aux<F>, Self) {
        let msg_len = self.get_register_value(REG_A1);
        let msg_ptr = self.get_register_value(REG_A2);
        let mut msg_vec = vec![];
        for addr in msg_ptr..(msg_ptr + msg_len) {
            msg_vec.push(self.load_u8(addr));
        }
        log::trace!(
            "VM TRACE LOG: {}",
            from_utf8(&msg_vec).expect("A valid utf8 VM trace log message should be provided")
        );
        (Aux::default(), self.bump_pc())
    }

    #[must_use]
    pub fn ecall(self) -> (Aux<F>, Self) {
        log::trace!(
            "ecall '{}' at clk: {}",
            ecall::log(self.get_register_value(REG_A0)),
            self.clk
        );
        match self.get_register_value(REG_A0) {
            ecall::HALT => self.ecall_halt(),
            ecall::IO_READ_PRIVATE => self.ecall_io_read(IoOpcode::StorePrivate),
            ecall::IO_READ_PUBLIC => self.ecall_io_read(IoOpcode::StorePublic),
            ecall::PANIC => self.ecall_panic(),
            ecall::POSEIDON2 => self.ecall_poseidon2(),
            ecall::VM_TRACE_LOG => self.ecall_trace_log(),
            _ => (Aux::default(), self.bump_pc()),
        }
    }
}
