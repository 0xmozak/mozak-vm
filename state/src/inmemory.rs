use std::hash::Hash;

use im::{HashMap, Vector};

use crate::storage::{Access, HistoricalLookup};

/// In-memory state storage with only the "current view" of the
/// state. Maintains no historical changes.
#[derive(Default, Clone)]
pub struct InMemoryStore<K, V>
where
    K: Clone + Default + Hash + Eq + PartialEq,
    V: Clone + Default, {
    /// Current state
    state: HashMap<K, V>,
}

/// In-memory historical state storage. This stores information
/// of all states from an arbitrary "starting height" (denoted
/// by a u64) till "current height". It can be safely assumed
/// that all heights [starting-height, current-height] can
/// be retrieved and only state at "current-height" can be modified.
/// All state information in [starting-height, current-height)
/// can be assumed to be immutable.
#[derive(Default, Clone)]
pub struct HistoricalInMemoryStore<K, V>
where
    K: Clone + Default + Hash + Eq + PartialEq,
    V: Clone + Default, {
    /// The oldest referencible state height
    start_height: u64,

    /// Current state
    current_state: InMemoryStore<K, V>,

    /// State storage for all heights [`start_height`, `current_height`)
    historical_states: Vector<InMemoryStore<K, V>>,
}

impl<K, V> HistoricalInMemoryStore<K, V>
where
    K: Clone + Default + Hash + Eq + PartialEq,
    V: Clone + Default,
{
    /// Initialize the `InMemoryStore` with a starting height
    #[must_use]
    pub fn new(start_height: u64) -> Self {
        Self {
            start_height,
            ..Default::default()
        }
    }
}

impl<K, V> Access<u64, K, V> for InMemoryStore<K, V>
where
    K: Clone + Default + Hash + Eq + PartialEq,
    V: Clone + Default,
{
    fn get(&self, key: &K) -> Option<&V> { self.state.get(key) }

    fn set(&mut self, key: K, value: V) -> Option<V> { self.state.insert(key, value) }
}

impl<K, V> Access<u64, K, V> for HistoricalInMemoryStore<K, V>
where
    K: Clone + Default + Hash + Eq + PartialEq,
    V: Clone + Default,
{
    fn get(&self, key: &K) -> Option<&V> { self.current_state.state.get(key) }

    fn set(&mut self, key: K, value: V) -> Option<V> { self.current_state.state.insert(key, value) }
}

impl<K, V> HistoricalLookup<u64, K, V> for HistoricalInMemoryStore<K, V>
where
    K: Clone + Default + Hash + Eq + PartialEq,
    V: Clone + Default,
{
    fn commit(&mut self) -> u64 {
        self.historical_states.push_back(self.current_state.clone());
        self.get_current_height()
    }

    fn get_start_height(&mut self) -> u64 { self.start_height }

    fn get_current_height(&mut self) -> u64 {
        self.start_height + self.historical_states.len() as u64
    }

    #[allow(clippy::cast_possible_truncation)]
    fn get_historical(&self, height: u64, key: K) -> Option<&V> {
        self.historical_states
            .get(height.checked_sub(self.start_height)? as usize)?
            .get(&key)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_and_set() {
        let (key, value) = ("RandomKey😊", "RandomValue😊");
        let mut ims = HistoricalInMemoryStore::new(6000);
        assert!(ims.get(&key).is_none());
        assert!(ims.set(key, value).is_none());
        assert_eq!(ims.set(key, value), Some(value));
        assert_eq!(ims.get(&key), Some(&value));
    }

    #[test]
    fn test_get_historical() {
        let (key, value, value_new) = ("RandomKey😊", "RandomValue😊", "ChangedValue😊");
        let mut ims = HistoricalInMemoryStore::new(6000);
        assert_eq!(ims.set(key, value), None);
        assert_eq!(ims.commit(), 6001);
        assert_eq!(ims.set(key, value_new), Some(value));
        assert_eq!(ims.get(&key), Some(&value_new));
        assert_eq!(ims.get_historical(6000, key), Some(&value));
    }
}
