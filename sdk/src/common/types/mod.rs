pub(crate) mod cross_program_call;
pub(crate) mod event;
pub(crate) mod poseidon2hash;
pub(crate) mod program_identifier;
pub(crate) mod raw_message;
pub(crate) mod state_address;
pub(crate) mod state_object;
pub(crate) mod system_tape;

pub use cross_program_call::CrossProgramCall;
pub use event::{Event, EventType};
pub use poseidon2hash::Poseidon2Hash;
pub use program_identifier::ProgramIdentifier;
pub use raw_message::RawMessage;
pub use state_address::StateAddress;
pub use state_object::StateObject;
pub use system_tape::{CallTapeType, EventTapeType, SystemTape};

// // #[cfg(not(target_os = "mozakvm"))]
// use itertools::chain;
// use rkyv::{AlignedVec, Archive, Deserialize, Serialize};

// #[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
// #[cfg_attr(
//     not(target_os = "mozakvm"),
//     derive(serde::Serialize, serde::Deserialize)
// )]
// #[archive(compare(PartialEq))]
// // #[cfg_attr(target_os = "mozakvm", derive(Debug))]
// #[archive_attr(derive(Debug))]
// // #[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]

// impl Default for CanonicalEventType {
//     fn default() -> Self { Self::Read }
// }

// #[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
// #[cfg_attr(
//     not(target_os = "mozakvm"),
//     derive(serde::Serialize, serde::Deserialize)
// )]
// #[archive(compare(PartialEq))]
// #[archive_attr(derive(Debug))]
// pub struct RawMessage(pub Vec<u8>);

// #[cfg(not(target_os = "mozakvm"))]
// impl std::fmt::Debug for RawMessage {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(
//             f,
//             "0x{}",
//             &self.iter().map(|x| hex::encode([*x])).collect::<String>()
//         )
//     }
// }

// impl core::ops::Deref for RawMessage {
//     type Target = Vec<u8>;

//     fn deref(&self) -> &Self::Target { &self.0 }
// }

// impl From<Vec<u8>> for RawMessage {
//     fn from(value: Vec<u8>) -> RawMessage { RawMessage(value) }
// }

// impl From<AlignedVec> for RawMessage {
//     fn from(value: AlignedVec) -> RawMessage { RawMessage(value.into_vec()) }
// }

// /// Canonical "address" type of object in "mozak vm".
// #[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Default, Clone)]
// #[cfg_attr(
//     not(target_os = "mozakvm"),
//     derive(serde::Serialize, serde::Deserialize)
// )]
// #[archive(compare(PartialEq))]
// #[archive_attr(derive(Debug))]
// #[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
// pub struct CPCMessage {
//     /// caller of cross-program-call message. Tuple of ProgramID
//     /// and methodID
//     /// TODO: Think about correctness of this??
//     pub caller_prog: ProgramIdentifier,

//     /// recipient of cross-program-call message. Tuple of ProgramID
//     /// and methodID
//     pub callee_prog: ProgramIdentifier,

//     /// raw message over cpc
//     pub args: RawMessage,
//     pub ret: RawMessage,
// }
