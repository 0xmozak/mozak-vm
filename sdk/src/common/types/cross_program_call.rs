use super::ProgramIdentifier;

#[derive(
    Default,
    Copy,
    Clone,
    Hash,
    PartialEq,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
/// `SelfCallExtension` is a boolean value that differentiates the
/// two program instances when a program calls itself under CPC. For
/// example, if a program `P` in some function `f` wants to call another
/// function `f'` in `P` under CPC regime, the caller and callee may form
/// tuple as: `((P, 0), (P, 1))` or `((P, 1), (P, 0))`. The added digits
/// help separate the two instances of call and hold no numeric significance
/// apart from mere differentiators.
pub struct SelfCallExtensionFlag(pub u8);

impl SelfCallExtensionFlag {
    /// Provides a flag different from given flag. In essence, if `0` is
    /// provided, `1` is returned and vice versa
    pub fn differentiate_from(flag: Self) -> Self { Self(1 - flag.0) }
}

#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct SelfCallExtendedProgramIdentifier(
    pub(crate) ProgramIdentifier,
    pub(crate) SelfCallExtensionFlag,
);

#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::pub_underscore_fields)]
pub struct CrossProgramCall {
    pub caller: SelfCallExtendedProgramIdentifier,
    pub callee: SelfCallExtendedProgramIdentifier,
    pub argument: super::RawMessage,
    pub return_: super::RawMessage,
}
