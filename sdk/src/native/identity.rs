#![allow(clippy::module_name_repetitions)]
use crate::common::types::RoleIdentifier;

/// Represents a stack for call contexts during native execution.
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IdentityStack(Vec<RoleIdentifier>);

impl IdentityStack {
    pub fn add_identity(&mut self, id: RoleIdentifier) { self.0.push(id); }

    #[must_use]
    pub fn top_identity(&self) -> RoleIdentifier { self.0.last().copied().unwrap_or_default() }

    pub fn rm_identity(&mut self) { self.0.truncate(self.0.len().saturating_sub(1)); }
}

/// Manually add a `RoleIdentifier` onto `IdentityStack`. Useful
/// when one want to escape automatic management of `IdentityStack`
/// via cross-program-calls sends (ideally temporarily).
/// CAUTION: Manual function for `IdentityStack`, misuse may lead
/// to system tape generation failure.
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub fn add_identity(id: crate::common::types::RoleIdentifier) {
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .identity_stack
            .borrow_mut()
            .add_identity(id);
    }
}

/// Manually remove a `RoleIdentifier` from `IdentityStack`.
/// Useful when one want to escape automatic management of `IdentityStack`
/// via cross-program-calls sends (ideally temporarily).
/// CAUTION: Manual function for `IdentityStack`, misuse may lead
/// to system tape generation failure.
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub fn rm_identity() {
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .identity_stack
            .borrow_mut()
            .rm_identity();
    }
}
