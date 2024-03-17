use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::common::traits::{EventEmit, SelfIdentify};
use crate::common::types::{CanonicalEvent, Event, ProgramIdentifier};
use crate::native::helpers::IdentityStack;

/// A list with ordered events according to either time
/// (temporal) or address & operations (canonical). Intenally
/// the elements are always kept in a temporal order; however
/// extraction is possible for both orderings.
#[derive(Default, Debug, Clone)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct OrderedEvents {
    temporal_ordering: Vec<(Event, CanonicalEvent)>,
}

impl OrderedEvents {
    pub fn new(emitter: ProgramIdentifier, events: Vec<Event>) -> Self {
        Self {
            temporal_ordering: events
                .into_iter()
                .map(|x| (x.clone(), CanonicalEvent::from_event(emitter, &x)))
                .collect(),
        }
    }

    /// Adds to ordered events an event "temporaly" a.k.a ordered in time
    /// after every other `Event` in `OrderedEvents`. This is the only
    /// way to add elements to `OrderedEvents`
    pub fn push_temporal(&mut self, emitter: ProgramIdentifier, event: Event) {
        let canonical_repr = CanonicalEvent::from_event(emitter, &event);
        self.temporal_ordering.push((event, canonical_repr));
    }

    /// Provides back a cononical ordering of events with attached indices
    /// pointing to the location of such `CanonicalEvent` in temporal
    /// ordering
    #[allow(dead_code)]
    fn get_canonical_ordering(&self) -> Vec<(CanonicalEvent, usize)> {
        let mut canonically_sorted = self
            .temporal_ordering
            .iter()
            .zip(0usize..)
            .map(|((_, canonical_event), idx)| (*canonical_event, idx))
            .collect::<Vec<(CanonicalEvent, usize)>>();
        canonically_sorted.sort();
        canonically_sorted
    }

    /// Returns a temporal order with hints on where to find elements
    /// w.r.t canonical order. Example:
    /// Temporal Order: [`Read_400`, `Read_200`, `Read_100`, `Read_300`]
    /// Canonical Hint: [   2,          1,           3,        0]
    #[allow(dead_code)]
    pub fn get_temporal_order_canonical_hints(&self) -> Vec<(Event, usize)> {
        self.temporal_ordering
            .iter()
            .zip(self.get_canonical_ordering())
            .map(|((event, _), (_, idx))| (event.clone(), idx))
            .collect::<Vec<(Event, usize)>>()
    }

    /// Returns a canonical order with hints on where to find elements
    /// w.r.t temporal order. Example:
    /// Temporal Order:  [`Read_400`, `Read_200`, `Read_100`, `Read_300`]
    /// Canonical Order: [`Read_100`, `Read_200`, `Read_300`, `Read_400`]
    /// Temporal Hints:  [   3,          1,           0,           2]
    #[allow(dead_code)]
    pub fn get_canonical_order_temporal_hints(&self) -> Vec<(CanonicalEvent, usize)> {
        fn reverse_ordering(original_ordering: &[usize]) -> Vec<usize> {
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
                .collect::<Vec<usize>>()
                .as_ref(),
        );

        canonical_ordering
            .into_iter()
            .zip(reversed_indices)
            .map(|((canonical_event, _), idx)| (canonical_event, idx))
            .collect::<Vec<(CanonicalEvent, usize)>>()
    }
}

/// Represents the `EventTape` under native execution
#[derive(Default, Clone)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct EventTape {
    #[serde(skip)]
    pub(crate) identity_stack: Rc<RefCell<IdentityStack>>,
    #[serde(rename = "individual_event_tapes")]
    pub(crate) writer: HashMap<ProgramIdentifier, OrderedEvents>,
}

impl std::fmt::Debug for EventTape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { self.writer.fmt(f) }
}

impl SelfIdentify for EventTape {
    fn set_self_identity(&mut self, _id: ProgramIdentifier) { unimplemented!() }

    fn get_self_identity(&self) -> ProgramIdentifier { self.identity_stack.borrow().top_identity() }
}

impl EventEmit for EventTape {
    fn emit(&mut self, event: Event) {
        let self_id = self.get_self_identity();
        assert_ne!(self_id, ProgramIdentifier::default());

        self.writer
            .entry(self_id)
            .and_modify(|x| x.push_temporal(self_id, event.clone()))
            .or_insert(OrderedEvents::new(self_id, vec![event]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::state_address::STATE_TREE_DEPTH;
    use crate::common::types::{EventType, StateAddress, StateObject};

    #[test]
    #[rustfmt::skip]
    fn test_ordered_events() {
        let common_emitter = ProgramIdentifier::new_from_rand_seed(1);
        let event1_read = Event{
            type_: EventType::Read,
            object: StateObject {
                address: StateAddress([1; STATE_TREE_DEPTH]),
                constraint_owner: ProgramIdentifier::new_from_rand_seed(2),
                data: vec![],
            }
        };
        let event2_read = Event{
            type_: EventType::Read,
            object: StateObject {
                address: StateAddress([2; STATE_TREE_DEPTH]),
                constraint_owner: ProgramIdentifier::new_from_rand_seed(3),
                data: vec![],
            }
        };
        let event3_read = Event{
            type_: EventType::Read,
            object: StateObject {
                address: StateAddress([3; STATE_TREE_DEPTH]),
                constraint_owner: ProgramIdentifier::new_from_rand_seed(4),
                data: vec![],
            }
        };

        let temporal_order = vec![event3_read.clone(), event1_read.clone(), event2_read.clone()];
        let expected_canonical_order = vec![
            CanonicalEvent::from_event(common_emitter, &event1_read),
            CanonicalEvent::from_event(common_emitter, &event2_read),
            CanonicalEvent::from_event(common_emitter, &event3_read)
        ];
        let expected_canonical_hints = vec![1, 2, 0];
        let expected_temporal_hints = vec![2, 0, 1];

        let ordered_events = OrderedEvents::new(common_emitter, temporal_order.clone());

        assert_eq!(ordered_events.get_canonical_order_temporal_hints(), 
            expected_canonical_order.into_iter().zip(expected_temporal_hints.into_iter()).collect::<Vec<(CanonicalEvent, usize)>>()); 
        
        assert_eq!(ordered_events.get_temporal_order_canonical_hints(), 
            temporal_order.into_iter().zip(expected_canonical_hints.into_iter()).collect::<Vec<(Event, usize)>>()); 
    }
}
