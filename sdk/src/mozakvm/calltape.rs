use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize};

use crate::common::traits::{Call, CallArgument, CallReturn, SelfIdentify};
use crate::common::types::{CrossProgramCall, ProgramIdentifier};

/// Represents the `CallTape` under `mozak-vm`
#[derive(Default, Clone)]
pub struct CallTape {
    pub(crate) cast_list: Vec<ProgramIdentifier>,
    pub(crate) self_prog_id: ProgramIdentifier,
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
    fn set_self_identity(&mut self, id: ProgramIdentifier) { self.self_prog_id = id; }

    fn get_self_identity(&self) -> ProgramIdentifier { self.self_prog_id }
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
        assert!(cpcmsg.caller == self.get_self_identity());
        assert!(cpcmsg.callee == recipient_program);
        assert!(self.is_casted_actor(&recipient_program));

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
        // before the end
        while self.index < self.reader.unwrap().len() {
            // Get the "archived" version of the message, where we will
            // pick and choose what we will deserialize
            let zcd_cpcmsg = &self.reader.unwrap()[self.index];

            // Mark this as "processed" regardless of what happens next.
            self.index += 1;

            // Well, once we are sure that we were not the caller, we can
            // either be a callee in which case we process and send information
            // back or we continue searching.
            let callee: ProgramIdentifier = zcd_cpcmsg
                .callee
                .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                .unwrap();

            if self.self_prog_id == callee {
                // First, ensure that we are not the caller, no-one can call
                // themselves. (Even if they can w.r.t. self-calling extension,
                // the `caller` field would remain distinct)
                let caller: ProgramIdentifier = zcd_cpcmsg
                    .caller
                    .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
                    .unwrap();
                assert!(caller != self.self_prog_id);

                // Before accepting, make sure that caller was a part of castlist
                assert!(self.is_casted_actor(&caller));

                let archived_args =
                    unsafe { rkyv::access_unchecked::<A>(zcd_cpcmsg.argument.0.as_slice()) };
                let args: A = archived_args.deserialize(Strategy::wrap(&mut ())).unwrap();

                let archived_ret =
                    unsafe { rkyv::access_unchecked::<R>(zcd_cpcmsg.return_.0.as_slice()) };
                let ret: R = archived_ret.deserialize(Strategy::wrap(&mut ())).unwrap();

                return Some((caller, args, ret));
            }
        }
        None
    }
}
