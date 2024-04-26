use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize};

use crate::common::traits::{Call, CallArgument, CallReturn, SelfIdentify};
use crate::common::types::{
    CrossProgramCall, ProgramIdentifier, SelfCallExtendedProgramIdentifier, SelfCallExtensionFlag,
};

/// Represents the `CallTape` under `mozak-vm`
#[derive(Default, Clone)]
pub struct CallTape {
    pub(crate) cast_list: Vec<ProgramIdentifier>,
    pub(crate) extended_self_prog_id: SelfCallExtendedProgramIdentifier,
    pub(crate) reader: Option<&'static <Vec<CrossProgramCall> as Archive>::Archived>,
    pub(crate) index: usize,
}

impl CallTape {
    /// Checks if actor seen is casted actor
    fn is_casted_actor(&self, actor: &ProgramIdentifier) -> bool {
        &ProgramIdentifier::default() == actor || self.cast_list.contains(actor)
    }
}

impl SelfIdentify for CallTape {
    fn get_self_identity(&self) -> SelfCallExtendedProgramIdentifier {
        self.extended_self_prog_id.clone()
    }
}

impl Call for CallTape {
    fn send<A, R>(
        &mut self,
        recipient_program: ProgramIdentifier,
        argument: A,
        _resolver: impl Fn(A) -> R,
    ) -> R
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
        <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
        // Ensure we aren't validating past the length of the event tape
        assert!(self.index < self.reader.unwrap().len());

        // Deserialize into rust form: CrossProgramCall.
        let zcd_cpcmsg = &self.reader.unwrap()[self.index];
        let cpcmsg: CrossProgramCall = zcd_cpcmsg
            .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
            .unwrap();

        // Ensure fields are correctly populated for caller and callee
        assert!(cpcmsg.caller.0 == self.get_self_identity().0);
        assert!(cpcmsg.callee.0 == recipient_program);
        assert!(self.is_casted_actor(&recipient_program));

        if self.get_self_identity().0 == recipient_program {
            assert!(cpcmsg.callee.1 == SelfCallExtensionFlag::differentiate_from(cpcmsg.caller.1));
        }

        // Deserialize the `arguments` seen on the tape, and assert
        let zcd_args = unsafe { rkyv::access_unchecked::<A>(&cpcmsg.argument.0[..]) };
        let deserialized_args =
            <<A as Archive>::Archived as Deserialize<A, Strategy<(), Panic>>>::deserialize(
                zcd_args,
                Strategy::wrap(&mut ()),
            )
            .unwrap();
        assert!(deserialized_args == argument);

        // Ensure we mark this index as "read"
        self.index += 1;

        // Return the claimed return value as seen on the tape
        // It remains that specific program's prerogative to ensure
        // that the return value used here is according to expectation
        let zcd_ret = unsafe { rkyv::access_unchecked::<R>(&cpcmsg.return_.0[..]) };
        <<R as Archive>::Archived as Deserialize<R, Strategy<(), Panic>>>::deserialize(
            zcd_ret,
            Strategy::wrap(&mut ()),
        )
        .unwrap()
    }

    #[allow(clippy::similar_names)]
    fn receive<A, R>(&mut self) -> Option<(ProgramIdentifier, A, R)>
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
        <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
        // Loop until we completely traverse the call tape in the
        // worst case. Hopefully, we see a message directed towards us
        // before the end.

        // SELF CALL EXTENSION: Looping to be done twice, once for
        // `(extended_self_prog_id, 0)` and once for `(extended_self_prog_id, 1)`
        let current_traversal_round: u8 = self.extended_self_prog_id.1 .0;
        if self.index >= self.reader.unwrap().len() && current_traversal_round == 0 {
            self.index = 0;
            self.extended_self_prog_id.1 =
                SelfCallExtensionFlag::differentiate_from(self.extended_self_prog_id.1);
        }

        while self.index < self.reader.unwrap().len() {
            // Get the "archived" version of the message, where we will
            // pick and choose what we will deserialize
            let zcd_cpcmsg = &self.reader.unwrap()[self.index];

            // Mark this as "processed" regardless of what happens next.
            self.index += 1;

            // Well, once we are sure that we were not the caller, we can
            // either be a callee in which case we process and send information
            // back or we continue searching.
            let callee: SelfCallExtendedProgramIdentifier = zcd_cpcmsg
                .callee
                .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                .unwrap();

            if callee == self.extended_self_prog_id {
                // Under self call extensions, a caller can call themselves, given
                // the `SelfCallExtensionFlag` is different between them
                let caller: SelfCallExtendedProgramIdentifier = zcd_cpcmsg
                    .caller
                    .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                    .unwrap();

                if caller.0 == callee.0 {
                    assert!(caller.1 == SelfCallExtensionFlag::differentiate_from(callee.1));
                }

                // Before accepting, make sure that caller was a part of castlist
                assert!(self.is_casted_actor(&caller.0));

                let archived_args =
                    unsafe { rkyv::access_unchecked::<A>(zcd_cpcmsg.argument.0.as_slice()) };
                let args: A = archived_args.deserialize(Strategy::wrap(&mut ())).unwrap();

                let archived_ret =
                    unsafe { rkyv::access_unchecked::<R>(zcd_cpcmsg.return_.0.as_slice()) };
                let ret: R = archived_ret.deserialize(Strategy::wrap(&mut ())).unwrap();

                return Some((caller.0, args, ret));
            }
        }
        None
    }
}
