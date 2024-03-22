#[cfg(target_os = "mozakvm")]
use crate::mozakvm as os;
#[cfg(not(target_os = "mozakvm"))]
use crate::native as os;

pub type CallTapeType = os::calltape::CallTape;
pub type EventTapeType = os::eventtape::EventTape;
pub type PublicInputTapeType = os::inputtape::PublicInputTape;
pub type PrivateInputTapeType = os::inputtape::PrivateInputTape;

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
