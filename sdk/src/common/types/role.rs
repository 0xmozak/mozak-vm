/// A monotonically increasing identifier of different program executions
/// based on the order of discover
pub type RoleIdentifier = u32;

/// A canonical identifier of different program executions
/// based on the address of programs
pub type CanonicalRoleIdentifier = u32;

#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct Role {
    pub object: super::StateObject,
}

#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct CanonicallyOrderedRoleIDsWithTemporalHints(pub CanonicalRoleIdentifier, pub u32);
