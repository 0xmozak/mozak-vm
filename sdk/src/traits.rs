use rkyv::ser::serializers::{
    AlignedSerializer, AllocScratch, CompositeSerializer, FallbackScratch, HeapScratch,
    SharedSerializeMap,
};
use rkyv::{AlignedVec, Archive, Deserialize};

use crate::coretypes::ProgramIdentifier;

pub trait RkyvSerializable = rkyv::Serialize<
    CompositeSerializer<
        AlignedSerializer<AlignedVec>,
        FallbackScratch<HeapScratch<256>, AllocScratch>,
        SharedSerializeMap,
    >,
>;
pub trait CallArgument = Sized + RkyvSerializable;
pub trait CallReturn = ?Sized + Clone + Default + RkyvSerializable + Archive;

/// A data struct that is aware of it's own ID
pub trait SelfIdentify {
    fn get_self_identity(&self) -> ProgramIdentifier;
    fn set_self_identity(&mut self, id: ProgramIdentifier);
}

/// `Call` trait provides methods `send` & `receive` to use an
/// underlying type as a message-passing system.
pub trait Call: SelfIdentify {
    /// `send` emulates a function call to the `resolver` with
    /// `argument` args and returns the value returned by it.
    /// Under the hood, wherever required, it uses `rkyv` for
    /// deserialization. This func never serializes.
    fn send<A, R>(
        &mut self,
        recepient_program: ProgramIdentifier,
        arguments: A,
        resolver: impl Fn(A) -> R,
    ) -> R
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as Archive>::Archived: Deserialize<A, rkyv::Infallible>,
        <R as Archive>::Archived: Deserialize<R, rkyv::Infallible>;

    /// `receive` emulates a function call directed towards the
    /// program, presents back with a three tuple of the form
    /// `(P, A, R)` where `P` is the identifier of the caller
    /// program, `A` the arguments they presented and `R` being
    /// the result that they want us to ensure is correct.
    /// Under the hood, wherever required, it uses `rkyv` for
    /// deserialization. This func never serializes.
    fn receive<A, R>(&mut self) -> Option<(ProgramIdentifier, A, R)>
    where
        A: CallArgument + PartialEq,
        R: CallReturn,
        <A as Archive>::Archived: Deserialize<A, rkyv::Infallible>,
        <R as Archive>::Archived: Deserialize<R, rkyv::Infallible>;
}
