use std::cell::RefCell;
use std::rc::Rc;

use rkyv::rancor::{Panic, Strategy};
use rkyv::Deserialize;

use crate::common::traits::{Call, CallArgument, CallReturn, SelfIdentify};
use crate::common::types::{CrossProgramCall, ProgramIdentifier, RawMessage};
use crate::native::helpers::IdentityStack;

/// Represents the `RawTape` under native execution
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawTape {
    #[serde(skip)]
    pub(crate) identity_stack: Rc<RefCell<IdentityStack>>,
    #[serde(rename = "individual_raw_tapes")]
    pub writer: HashMap<ProgramIdentifier, Vec<u8>>,
}

impl std::fmt::Debug for RawTape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.writer.fmt(f) }
}

impl SelfIdentify for RawTape {
    fn set_self_identity(&mut self, id: ProgramIdentifier) {
        self.identity_stack.borrow_mut().add_identity(id);
    }

    fn get_self_identity(&self) -> ProgramIdentifier { self.identity_stack.borrow().top_identity() }
}

/// We have to implement `std::io::Write` in native context
/// to infact "write" elements onto RawTape. In native context
/// this should always be available and is not bound by
/// `stdread` or any other feature flag.
impl std::io::Write for RawTape {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let self_id = self.get_self_identity();
        assert_ne!(self_id, ProgramIdentifier::default());

        self.writer
            .entry(self_id)
            .and_modify(|x| x.push(buf))
            .or_insert(Vec::from(buf));

        buf.len()
    }

    // Flush is a no-op
    fn flush(&mut self) -> Result<()> {Ok(())}
}

pub struct PrivateInputTape(RawTape);
pub struct PublicInputTape(RawTape);
