// These symbols are populated at link time due to
// linker script
extern "C" {
    pub static _mozak_self_prog_id: usize;
    pub static _mozak_cast_list: usize;
    pub static _mozak_public_tape: usize;
    pub static _mozak_private_tape: usize;
    pub static _mozak_call_tape: usize;
    pub static _mozak_event_tape: usize;
}
