use rkyv::{Archive, Deserialize};

use crate::coretypes::{CPCMessage, ProgramIdentifier};
use crate::traits::{Call, SelfIdentify};

/// Represents the `CallTape` under `mozak-vm`
pub struct CallTapeMozakVM {
    cast_list: Vec<ProgramIdentifier>,
    self_prog_id: ProgramIdentifier,
    reader: Option<&'static <Vec<CPCMessage> as Archive>::Archived>,
    index: usize,
}

impl CallTapeMozakVM {
    /// Checks if actor seen is casted actor
    fn is_casted_actor(&self, actor: &ProgramIdentifier) -> bool {
        &ProgramIdentifier::default() == actor || self.cast_list.contains(actor)
    }
}

impl SelfIdentify for CallTapeMozakVM {
    fn set_self_identity(&mut self, id: ProgramIdentifier) { self.self_prog_id = id; }

    fn get_self_identity(&self) -> &ProgramIdentifier { &self.self_prog_id }
}

impl Call for CallTapeMozakVM {
    fn send<A, R>(
        &mut self,
        recepient_program: crate::coretypes::ProgramIdentifier,
        arguments: A,
        _resolver: impl Fn(A) -> R,
    ) -> R
    where
        A: crate::traits::CallArgument + PartialEq,
        R: crate::traits::CallReturn,
        <A as rkyv::Archive>::Archived: rkyv::Deserialize<A, rkyv::Infallible>,
        <R as rkyv::Archive>::Archived: rkyv::Deserialize<R, rkyv::Infallible>, {
        // Ensure we aren't validating past the length of the event tape
        assert!(self.index < self.reader.unwrap().len());

        // Deserialize into rust form: CPCMessage.
        let zcd_cpcmsg = &self.reader.unwrap()[self.index];
        let cpcmsg: CPCMessage = zcd_cpcmsg.deserialize(&mut rkyv::Infallible).unwrap();

        // Ensure fields are correctly populated for caller and callee
        assert_eq!(cpcmsg.caller_prog, *self.get_self_identity());
        assert_eq!(cpcmsg.callee_prog, recepient_program);
        assert!(self.is_casted_actor(&recepient_program));

        // Deserialize the `arguments` seen on the tape, and assert
        let zcd_args = unsafe { rkyv::archived_root::<A>(&cpcmsg.args.0[..]) };
        let deserialized_args =
            <<A as Archive>::Archived as Deserialize<A, rkyv::Infallible>>::deserialize(
                zcd_args,
                &mut rkyv::Infallible,
            )
            .unwrap();
        assert!(deserialized_args == arguments);

        // Ensure we mark this index as "read"
        self.index += 1;

        // Return the claimed return value as seen on the tape
        // It remains that specific program's prerogative to ensure
        // that the return value used here is according to expectation
        let zcd_ret = unsafe { rkyv::archived_root::<R>(&cpcmsg.ret.0[..]) };
        <<R as Archive>::Archived as Deserialize<R, rkyv::Infallible>>::deserialize(
            zcd_ret,
            &mut rkyv::Infallible,
        )
        .unwrap()
    }

    #[allow(clippy::similar_names)]
    fn receive<A, R>(&mut self) -> Option<(crate::coretypes::ProgramIdentifier, A, R)>
    where
        A: crate::traits::CallArgument + PartialEq,
        R: crate::traits::CallReturn,
        <A as rkyv::Archive>::Archived: rkyv::Deserialize<A, rkyv::Infallible>,
        <R as rkyv::Archive>::Archived: rkyv::Deserialize<R, rkyv::Infallible>, {
        // Loop until we completely traverse the call tape in the
        // worst case. Hopefully, we see a message directed towards us
        // before the end
        while self.index < self.reader.unwrap().len() {
            // Get the "archived" version of the message, where we will
            // pick and choose what we will deserialize
            let zcd_cpcmsg = &self.reader.unwrap()[self.index];

            // Mark this as "processed" regardless of what happens next.
            self.index += 1;

            // First, ensure that we are not the caller, no-one can call
            // themselves. (Even if they can w.r.t. self-calling extension,
            // the `caller` field would remain distinct)
            let caller: ProgramIdentifier = zcd_cpcmsg
                .caller_prog
                .deserialize(&mut rkyv::Infallible)
                .unwrap();
            assert_ne!(caller, self.self_prog_id);

            // Well, once we are sure that we were not the caller, we can
            // either be a callee in which case we process and send information
            // back or we continue searching.
            let callee: ProgramIdentifier = zcd_cpcmsg
                .callee_prog
                .deserialize(&mut rkyv::Infallible)
                .unwrap();

            if self.self_prog_id == callee {
                // Before accepting, make sure that caller was a part of castlist
                assert!(self.is_casted_actor(&caller));

                let archived_args =
                    unsafe { rkyv::archived_root::<A>(zcd_cpcmsg.args.0.as_slice()) };
                let args: A = archived_args.deserialize(&mut rkyv::Infallible).unwrap();

                let archived_ret = unsafe { rkyv::archived_root::<R>(zcd_cpcmsg.ret.0.as_slice()) };
                let ret: R = archived_ret.deserialize(&mut rkyv::Infallible).unwrap();

                return Some((caller, args, ret));
            }
        }
        None
    }
}
