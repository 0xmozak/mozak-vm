use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::{Failure, Panic, Strategy};
use rkyv::ser::allocator::{AllocationTracker, GlobalAllocator};
use rkyv::ser::{AllocSerializer, Composite};
use rkyv::util::AlignedVec;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{Archive, Deserialize, Serialize};

use crate::common::types::{Event, ProgramIdentifier};

pub trait RkyvSerializable = rkyv::Serialize<
        Strategy<Composite<AlignedVec, AllocationTracker<GlobalAllocator>, Panic>, Panic>,
    > + Serialize<Strategy<AllocSerializer<256>, Panic>>;
pub trait CallArgument = Sized + RkyvSerializable;
pub trait ArchivedCallArgument<A> =
    Deserialize<A, Strategy<(), Panic>> + CheckBytes<Strategy<DefaultValidator, Failure>>;
pub trait ArchivedCallReturn<R> =
    Deserialize<R, Strategy<(), Panic>> + CheckBytes<Strategy<DefaultValidator, Failure>>;

pub trait CallReturn = ?Sized + Clone + Default + RkyvSerializable + Archive;

/// A data struct that is aware of it's own ID
pub trait SelfIdentify {
    fn get_self_identity(&self) -> ProgramIdentifier;
    #[allow(dead_code)]
    fn set_self_identity(&mut self, id: ProgramIdentifier);
}

/// `Call` trait provides methods `send` & `receive` to use an
/// underlying type as a message-passing system.
pub trait Call: SelfIdentify {
    /// `send` emulates a function call to the `resolver` with
    /// `argument` args and returns the value returned by it.
    /// Under the hood, wherever required, it uses `rkyv` for
    /// deserialization. This func never serializes in `mozakvm`.
    fn send<A, R>(
        &mut self,
        recipient_program: ProgramIdentifier,
        argument: A,
        resolver: impl Fn(A) -> R,
    ) -> R
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as Archive>::Archived: ArchivedCallArgument<A>,
        <R as Archive>::Archived: ArchivedCallReturn<R>;

    /// `receive` emulates a function call directed towards the
    /// program, presents back with a three tuple of the form
    /// `(P, A, R)` where `P` is the identifier of the caller
    /// program, `A` the arguments they presented and `R` being
    /// the result that they want us to ensure is correct.
    /// Under the hood, wherever required, it uses `rkyv` for
    /// deserialization. This func never serializes in `mozakvm`.
    fn receive<A, R>(&mut self) -> Option<(ProgramIdentifier, A, R)>
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as Archive>::Archived: ArchivedCallArgument<A>,
        <R as Archive>::Archived: ArchivedCallReturn<R>;
}

/// `EventEmit` trait provides method `emit` to use the underlying
/// tape as an output device
pub trait EventEmit: SelfIdentify {
    /// `emit` emulates an output device write
    fn emit(&mut self, event: Event);
}
