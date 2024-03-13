use std::cell::RefCell;

use crate::native_helpers::IdentityStack;
use crate::traits::{Call, SelfIdentify};
use crate::types::{CPCMessage, ProgramIdentifier, RawMessage};

/// Represents the `CallTape` under native execution
#[derive(Default)]
pub struct CallTapeNative {
    pub identity_stack: RefCell<IdentityStack>,
    pub writer: Vec<CPCMessage>,
}

impl SelfIdentify for CallTapeNative {
    fn set_self_identity(&mut self, id: ProgramIdentifier) {
        self.identity_stack.borrow_mut().add_identity(id);
    }

    fn get_self_identity(&self) -> ProgramIdentifier { self.identity_stack.borrow().top_identity() }
}

impl Call for CallTapeNative {
    fn send<A, R>(
        &mut self,
        recepient_program: crate::types::ProgramIdentifier,
        arguments: A,
        resolver: impl Fn(A) -> R,
    ) -> R
    where
        A: crate::traits::CallArgument + PartialEq,
        R: crate::traits::CallReturn,
        <A as rkyv::Archive>::Archived: rkyv::Deserialize<A, rkyv::Infallible>,
        <R as rkyv::Archive>::Archived: rkyv::Deserialize<R, rkyv::Infallible>, {
        // Create a skeletal `CPCMessage` to be resolved via "resolver"
        let msg = CPCMessage {
            caller_prog: self.get_self_identity(),
            callee_prog: recepient_program,
            args: rkyv::to_bytes::<_, 256>(&arguments).unwrap().into(),
            ret: RawMessage::default(), // Unfilled: we have to still resolve it
        };

        // Remember where in the "writer" are we pushing this.
        // This is needed since during the time we spend resolving this
        // `CPCMessage`, other elements would be added onto "writer"
        let inserted_idx = self.writer.len();

        // and... insert
        self.writer.push(msg);

        // resolve the return value and add to where message was
        self.set_self_identity(recepient_program);
        let resolved_value = resolver(arguments);
        self.writer[inserted_idx].ret = rkyv::to_bytes::<_, 256>(&resolved_value).unwrap().into();
        self.identity_stack.borrow_mut().rm_identity();

        resolved_value
    }

    fn receive<A, R>(&mut self) -> Option<(crate::types::ProgramIdentifier, A, R)>
    where
        A: crate::traits::CallArgument + PartialEq,
        R: crate::traits::CallReturn,
        <A as rkyv::Archive>::Archived: rkyv::Deserialize<A, rkyv::Infallible>,
        <R as rkyv::Archive>::Archived: rkyv::Deserialize<R, rkyv::Infallible>, {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use crate::call_tape_native::CallTapeNative;
    use crate::traits::Call;
    use crate::types::ProgramIdentifier;

    fn test_pid_generator(val: u8) -> ProgramIdentifier {
        let mut pid = ProgramIdentifier::default();
        pid.0 .0[0] = val;
        pid
    }

    #[test]
    fn test_send_native_single_call() {
        type A = u8;
        type B = u16;

        let mut calltape = CallTapeNative::default();

        let resolver = |val: A| -> B { (val + 1) as B };

        let response = calltape.send(test_pid_generator(1), 1 as A, resolver);
        assert_eq!(response, 2);
        assert_eq!(calltape.writer.len(), 1);
        assert_eq!(calltape.writer[0].caller_prog, ProgramIdentifier::default());
        assert_eq!(calltape.writer[0].callee_prog, test_pid_generator(1));
    }
}
