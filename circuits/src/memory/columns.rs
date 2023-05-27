

pub struct MemoryColumns<T> {
    /// Memory address
    pub addr: T,

    /// Memory cell
    pub value: T,

    /// Main CPU clock cycle
    pub clk: T,

    /// Whether memory operation is a read
    pub is_read: T,

    /// Whether memory operation is a real read, not a dummy.
    pub is_real: T,

    /// Either addr' - addr (if address is changed), or clk' - clk (if address is not changed)
    pub diff: T,
    /// The inverse of `diff`, or 0 if `diff = 0`.
    pub diff_inv: T,

    /// A boolean flag indicating whether addr' - addr == 0
    pub addr_not_equal: T,

    pub counter: T,
}
