use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rkyv::rancor::{Panic, Strategy};
use rkyv::Deserialize;

use crate::common::traits::{Call, CallArgument, CallReturn, SelfIdentify};
use crate::common::types::{CrossProgramCall, ProgramIdentifier, RawMessage, RoleIdentifier};
use crate::native::identity::IdentityStack;

/// Represents the `CallTape` under native execution
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallTape {
    #[serde(skip)]
    pub next_available_role_id: RoleIdentifier,
    #[serde(skip)]
    pub lookup_role_map: HashMap<(ProgramIdentifier, String), RoleIdentifier>,
    #[serde(skip)]
    pub unique_role_map: HashMap<ProgramIdentifier, RoleIdentifier>,
    #[serde(skip)]
    pub(crate) identity_stack: Rc<RefCell<IdentityStack>>,
    #[serde(rename = "global_calltape")]
    pub writer: Vec<CrossProgramCall>,
}

impl std::fmt::Debug for CallTape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.writer.fmt(f) }
}

impl SelfIdentify for CallTape {
    fn set_self_identity(&mut self, id: RoleIdentifier) {
        self.identity_stack.borrow_mut().add_identity(id);
    }

    fn get_self_identity(&self) -> RoleIdentifier { self.identity_stack.borrow().top_identity() }
}

impl Call for CallTape {
    fn send<A, R>(
        &mut self,
        recipient: RoleIdentifier,
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
            callee: recipient,
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
        self.set_self_identity(recipient);
        let resolved_value = resolver(argument);
        self.writer[inserted_idx].return_ =
            rkyv::to_bytes::<_, 256, _>(&resolved_value).unwrap().into();
        self.identity_stack.borrow_mut().rm_identity();

        resolved_value
    }

    fn receive<A, R>(&mut self) -> Option<(RoleIdentifier, A, R)>
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
        <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
        unimplemented!()
    }
}

impl CallTape {
    /// Gets a roleID determined fully by `(Prog, instance)` tuple. It is
    /// guaranteed that any call wih same `(Prog, instance)` tuple during one
    /// native context will always return the same `RoleIdentifier` within that
    /// context. Useful when different programs need to call the same role.
    pub fn get_deterministic_role_id(&mut self, prog: ProgramIdentifier, instance: String) -> RoleIdentifier {
        let identifier = (prog, instance);

        if let Some(role_id) = self.lookup_role_map.get(&identifier) {
            return *role_id
        };

        // if not already found in role map
        let allocated_role = self.next_available_role_id;
        self.next_available_role_id += 1;

        self.lookup_role_map.insert(identifier, allocated_role);
        allocated_role
    }

    /// Gets a fresh & unique roleID referencible only by the `RoleIdentifier`
    pub fn get_unique_role_id(&mut self, prog: ProgramIdentifier) -> RoleIdentifier {
        let allocated_role = self.next_available_role_id;
        self.next_available_role_id += 1;

        self.unique_role_map.insert(prog, allocated_role);

        allocated_role
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
