// These symbols are populated at link time due to
// linker script
extern "C" {
    pub static mozak_self_prog_id: usize;
    pub static mozak_cast_list: usize;
    // pub static mozak_public_io_tape: usize;
    // pub static mozak_private_io_tape: usize;
    pub static mozak_call_tape: usize;
    // pub static mozak_event_tape: usize;
}
