use crate::coretypes::{CPCMessage, ProgramIdentifier, RawMessage};
use crate::traits::{Call, SelfIdentify};

/// Represents the `CallTape` under native execution
pub struct CallTapeNative {
    pub identity_stack: Vec<ProgramIdentifier>,
    pub writer: Vec<CPCMessage>,
}

impl SelfIdentify for CallTapeNative {
    fn set_self_identity(&mut self, _id: ProgramIdentifier) { unimplemented!() }

    fn get_self_identity(&self) -> ProgramIdentifier { 
        // returns the "latest" identity
        self.identity_stack.last().copied().unwrap_or_default()
    }
}

impl Call for CallTapeNative {
    fn send<A, R>(
        &mut self,
        recepient_program: crate::coretypes::ProgramIdentifier,
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
        self.identity_stack.push(recepient_program);
        let resolved_value = resolver(arguments);
        self.writer[inserted_idx].ret = rkyv::to_bytes::<_, 256>(&resolved_value).unwrap().into();
        self.identity_stack.truncate(self.identity_stack.len().saturating_sub(1));

        resolved_value
    }

    fn receive<A, R>(&mut self) -> Option<(crate::coretypes::ProgramIdentifier, A, R)>
    where
        A: crate::traits::CallArgument + PartialEq,
        R: crate::traits::CallReturn,
        <A as rkyv::Archive>::Archived: rkyv::Deserialize<A, rkyv::Infallible>,
        <R as rkyv::Archive>::Archived: rkyv::Deserialize<R, rkyv::Infallible>, {
        unimplemented!()
    }
}
