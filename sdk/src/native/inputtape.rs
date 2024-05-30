use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::common::traits::SelfIdentify;
use crate::common::types::{RawMessage, RoleIdentifier};
use crate::native::identity::IdentityStack;

/// Represents the `RawTape` under native execution
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawTape {
    #[serde(skip)]
    pub(crate) identity_stack: Rc<RefCell<IdentityStack>>,
    #[serde(rename = "individual_raw_tapes")]
    pub writer: HashMap<RoleIdentifier, RawMessage>,
}

impl std::fmt::Debug for RawTape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.writer.fmt(f) }
}

impl SelfIdentify for RawTape {
    fn set_self_identity(&mut self, id: RoleIdentifier) {
        self.identity_stack.borrow_mut().add_identity(id);
    }

    fn get_self_identity(&self) -> RoleIdentifier { self.identity_stack.borrow().top_identity() }
}

/// We have to implement `std::io::Write` in native context
/// to infact "write" elements onto `RawTape`. In native context
/// this should always be available and is not bound by
/// `stdread` or any other feature flag.
impl std::io::Write for RawTape {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let self_id = self.get_self_identity();
        assert_ne!(self_id, RoleIdentifier::default());

        self.writer.entry(self_id).or_default().0.extend(buf);

        Ok(buf.len())
    }

    // Flush is a no-op
    fn flush(&mut self) -> Result<(), std::io::Error> { Ok(()) }
}

pub type PrivateInputTape = RawTape;
pub type PublicInputTape = RawTape;

#[allow(clippy::missing_errors_doc)]
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub fn write(kind: &crate::InputTapeType, buf: &[u8]) -> std::io::Result<usize> {
    use std::io::Write;
    match kind {
        crate::InputTapeType::PublicTape => unsafe {
            crate::common::system::SYSTEM_TAPE
                .public_input_tape
                .write(buf)
        },
        crate::InputTapeType::PrivateTape => unsafe {
            crate::common::system::SYSTEM_TAPE
                .private_input_tape
                .write(buf)
        },
    }
}
