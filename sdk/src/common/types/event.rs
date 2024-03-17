#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
#[repr(u8)]
pub enum EventType {
    Read = 0,
    Write,
    Ensure,
    Create,
    Delete,
}

impl Default for EventType {
    fn default() -> Self { Self::Read }
}

// Common derives
#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct Event {
    pub object: super::StateObject,
    pub type_: EventType,
}

// Common derives
#[derive(
    Default,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
// Derives only for non-mozakvm
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
pub struct CanonicalEvent {
    pub address: super::StateAddress,
    pub type_: EventType,
    pub value: super::Poseidon2Hash,
    pub emitter: super::ProgramIdentifier,
}

#[cfg(not(target_os = "mozakvm"))]
impl CanonicalEvent {
    #[must_use]
    pub fn from_event(emitter: super::ProgramIdentifier, value: &Event) -> Self {
        Self {
            address: value.object.address,
            type_: value.type_,
            value: crate::native::helpers::poseidon2_hash(&value.object.data),
            emitter,
        }
    }
}

// /// Ordering of events: sorted according to time or according
// /// to address & operation
// #[cfg(not(target_os = "mozakvm"))]
// pub enum Ordering {
//     /// ordering according to time as they occur in execution
//     Temporal,
//     /// ordering according to address & operation of state object
//     Canonical,
// }

/// A list with ordered events according to either time
/// (temporal) or address & operations (canonical). Intenally
/// the elements are always kept in a temporal order; however
/// extraction is possible for both orderings.
#[cfg(not(target_os = "mozakvm"))]
pub struct OrderedEvents {
    temporal_ordering: Vec<(Event, CanonicalEvent)>,
}

#[cfg(not(target_os = "mozakvm"))]
impl OrderedEvents {
    /// Adds to ordered events an event "temporaly" a.k.a ordered in time
    /// after every other `Event` in `OrderedEvents`. This is the only
    /// way to add elements to `OrderedEvents`
    pub fn push_temporal(&mut self, emitter: super::ProgramIdentifier, event: Event) {
        let canonical_repr = CanonicalEvent::from_event(emitter, &event);
        self.temporal_ordering.push((event, canonical_repr));
    }

    /// Provides back a cononical ordering of events with attached indices
    /// pointing to the location of such `CanonicalEvent` in temporal
    /// ordering
    pub fn get_canonical_ordering(&self) -> Vec<(CanonicalEvent, usize)> {
        let mut canonically_sorted = self
            .temporal_ordering
            .iter()
            .zip(0usize..)
            .map(|((_, canonical_event), idx)| (canonical_event.clone(), idx))
            .collect::<Vec<(CanonicalEvent, usize)>>();
        canonically_sorted.sort();
        canonically_sorted
    }

    /// Returns a temporal order with hints on where to find elements
    /// w.r.t canonical order. Example:
    /// Temporal Order: [`Read_400`, `Read_200`, `Read_100`, `Read_300`]
    /// Canonical Hint: [   2,          1,           3,        0]
    pub fn get_temporal_order_canonical_hints(&self) -> Vec<(Event, usize)> {
        self.temporal_ordering
            .iter()
            .zip(self.get_canonical_ordering())
            .map(|((event, _), (_, idx))| (event.clone(), idx))
            .collect::<Vec<(Event, usize)>>()
    }

    /// Returns a canonical order with hints on where to find elements
    /// w.r.t temporal order. Example:
    /// Temporal Order: [`Read_400`, `Read_200`, `Read_100`, `Read_300`]
    /// Canonical Order: [`Read_100'`, `Read_200'`, `Read_300'`, `Read_400'`]
    /// Temporal Hints: [   3,       1,       0,        2]
    pub fn get_canonical_order_temporal_hints(&self) -> Vec<(CanonicalEvent, usize)> {
        fn reverse_ordering(original_ordering: Vec<usize>) -> Vec<usize> {
            let mut reversed_ordering = vec![0; original_ordering.len()];

            // Iterate through the original ordering
            for (index, &position) in original_ordering.iter().enumerate() {
                reversed_ordering[position] = index;
            }

            reversed_ordering
        }

        let canonical_ordering = self.get_canonical_ordering();

        let reversed_indices = reverse_ordering(
            canonical_ordering
                .iter()
                .map(|(_, idx)| *idx)
                .collect::<Vec<usize>>(),
        );

        canonical_ordering
            .into_iter()
            .zip(reversed_indices)
            .map(|((canonical_event, _), idx)| (canonical_event.clone(), idx))
            .collect::<Vec<(CanonicalEvent, usize)>>()
    }
}
