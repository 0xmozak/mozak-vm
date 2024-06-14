use std::cell::RefCell;
use std::rc::Rc;

use rkyv::rancor::{Panic, Strategy};
use rkyv::Deserialize;

use crate::common::traits::{Call, CallArgument, CallReturn, SelfIdentify};
use crate::common::types::{CrossProgramCall, ProgramIdentifier, RawMessage};
use crate::native::identity::IdentityStack;

/// Represents the `CallTape` under native execution
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallTape {
    #[serde(skip)]
    pub(crate) identity_stack: Rc<RefCell<IdentityStack>>,
    #[serde(rename = "global_calltape")]
    pub writer: Vec<CrossProgramCall>,
}

impl std::fmt::Debug for CallTape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.writer.fmt(f) }
}

impl SelfIdentify for CallTape {
    fn set_self_identity(&mut self, id: ProgramIdentifier) {
        self.identity_stack.borrow_mut().add_identity(id);
    }

    fn get_self_identity(&self) -> ProgramIdentifier { self.identity_stack.borrow().top_identity() }
}

impl Call for CallTape {
    fn send<A, R>(
        &mut self,
        recipient_program: ProgramIdentifier,
        argument: A,
        resolver: impl Fn(A) -> R,
    ) -> R
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
        <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
        // Create a skeletal `CrossProgramCall` to be resolved via "resolver"
        let msg = CrossProgramCall {
            caller: self.get_self_identity(),
            callee: recipient_program,
            argument: rkyv::to_bytes::<_, 256, _>(&argument).unwrap().into(),
            return_: RawMessage::default(), // Unfilled: we have to still resolve it
        };

        // Remember where in the "writer" are we pushing this.
        // This is needed since during the time we spend resolving this
        // `CrossProgramCall`, other elements would be added onto "writer"
        let inserted_idx = self.writer.len();

        // and... insert
        self.writer.push(msg);

        // resolve the return value and add to where message was
        self.set_self_identity(recipient_program);
        let resolved_value = resolver(argument);
        self.writer[inserted_idx].return_ =
            rkyv::to_bytes::<_, 256, _>(&resolved_value).unwrap().into();
        self.identity_stack.borrow_mut().rm_identity();

        resolved_value
    }

    fn receive<A, R>(&mut self) -> Option<(ProgramIdentifier, A, R)>
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
        <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
            return None;
        // unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::CallTape;
    use crate::common::traits::Call;
    use crate::common::types::ProgramIdentifier;

    fn test_pid_generator(val: u8) -> ProgramIdentifier {
        let mut pid = ProgramIdentifier::default();
        pid.0 .0[0] = val;
        pid
    }

    #[test]
    fn test_send_native_single_call() {
        type A = u8;
        type B = u16;

        let mut calltape = CallTape::default();

        let resolver = |val: A| -> B { B::from(val + 1) };

        let response = calltape.send(test_pid_generator(1), 1 as A, resolver);
        assert_eq!(response, 2);
        assert_eq!(calltape.writer.len(), 1);
        assert_eq!(calltape.writer[0].caller, ProgramIdentifier::default());
        assert_eq!(calltape.writer[0].callee, test_pid_generator(1));
    }
}
