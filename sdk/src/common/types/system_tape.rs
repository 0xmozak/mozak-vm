#[cfg(target_os = "mozakvm")]
pub type CallTapeType = crate::mozakvm::calltape::CallTape;
#[cfg(not(target_os = "mozakvm"))]
pub type CallTapeType = crate::native::calltape::CallTape;

#[cfg(target_os = "mozakvm")]
pub type EventTapeType = crate::mozakvm::eventtape::EventTape;
#[cfg(not(target_os = "mozakvm"))]
pub type EventTapeType = crate::native::eventtape::EventTape;

#[derive(Default, Clone)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
pub struct SystemTape {
    // TODO: Add Public and Private IO Tape
    pub call_tape: CallTapeType,
    pub event_tape: EventTapeType,
}
