/// Allows for any state store to provide
/// read-write and commit access for the current state.
/// We assume monotonically increasing (by 1) heights and
/// hence also provides read access to heights readable
/// via the state store.
pub trait Access<H, K, V> {
    /// Get the value corresponding to given key at the current height.
    fn get(&self, key: &K) -> Option<&V>;

    /// Set the value corresponding to given key at the current height.
    /// Returns previously set value if any.
    fn set(&mut self, key: K, value: V) -> Option<V>;
}

/// Allows for any state store to provide historical
/// read access a.k.a in [`start_height`, `current_height`)
pub trait HistoricalLookup<H, K, V> {
    /// Mark current height as immutable, and preserve the history,
    /// increases height.
    fn commit(&mut self) -> H;

    /// Get the oldest referencible state height
    fn get_start_height(&mut self) -> H;

    /// Get the current state height
    ///
    /// The state height currently un-committed. Only this height
    /// can be modified. Post `commit()` this increases by `1`.
    /// [`start_height`, `current_height`) is immutable.
    fn get_current_height(&mut self) -> H;

    /// Get the value corresponding to given key at the requested height.
    /// Height should be within [`start_height`, `current_height`)
    fn get_historical(&self, height: H, key: K) -> Option<&V>;
}
