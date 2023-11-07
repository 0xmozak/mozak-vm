/// A view into the unique value and its multiplicity. Used in
/// logUp subset check.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MultiplicityView<T> {
    /// The unique value.
    pub value: T,

    /// The frequencies for which the accompanying value occur in
    /// the trace. This is m(x) in the paper.
    pub multiplicity: T,
}
