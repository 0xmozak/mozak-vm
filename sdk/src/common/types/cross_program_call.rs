#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[archive(check_bytes)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[archive_attr(derive(Debug))]
#[allow(clippy::pub_underscore_fields)]
pub struct CrossProgramCall {
    pub caller: super::ProgramIdentifier,
    pub callee: super::ProgramIdentifier,
    pub argument: super::RawMessage,
    pub return_: super::RawMessage,
}
