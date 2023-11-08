/// A view into the unique value and its multiplicity. Used in
/// logUp subset check.
/// TODO: allow tuple of values too ?
///       we can avoid extra column for `value` if we can use tuple of limbs
/// there.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct MultiplicityView<T> {
    /// The unique value.
    pub value: T,

    /// The frequencies for which the accompanying value occur in
    /// the trace. This is m(x) in the paper.
    pub multiplicity: T,
}
