#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::pub_underscore_fields)]
pub struct CrossProgramCall {
    pub caller: super::RoleIdentifier,
    pub callee: super::RoleIdentifier,
    pub argument: super::RawMessage,
    pub return_: super::RawMessage,
}
