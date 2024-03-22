#[cfg(target_os = "mozakvm")]
pub type CallTapeType = crate::mozakvm::calltape::CallTape;
#[cfg(not(target_os = "mozakvm"))]
pub type CallTapeType = crate::native::calltape::CallTape;

#[cfg(target_os = "mozakvm")]
pub type EventTapeType = crate::mozakvm::eventtape::EventTape;
#[cfg(not(target_os = "mozakvm"))]
pub type EventTapeType = crate::native::eventtape::EventTape;

#[cfg(target_os = "mozakvm")]
pub type PublicInputTapeType = crate::mozakvm::inputtape::PublicInputTape;
#[cfg(not(target_os = "mozakvm"))]
pub type PublicInputTapeType = crate::native::inputtape::PublicInputTape;

#[cfg(target_os = "mozakvm")]
pub type PrivateInputTapeType = crate::mozakvm::inputtape::PrivateInputTape;
#[cfg(not(target_os = "mozakvm"))]
pub type PrivateInputTapeType = crate::native::inputtape::PrivateInputTape;

#[derive(Default, Clone)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
pub struct SystemTape {
    pub private_input_tape: PrivateInputTapeType,
    pub public_input_tape: PublicInputTapeType,
    pub call_tape: CallTapeType,
    pub event_tape: EventTapeType,
}
