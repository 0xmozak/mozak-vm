// Common derives
#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::pub_underscore_fields)]
pub struct CrossProgramCall {
    pub caller: super::ProgramIdentifier,
    pub callee: super::ProgramIdentifier,
    pub argument: super::RawMessage,
    pub return_: super::RawMessage,
}
