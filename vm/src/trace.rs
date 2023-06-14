/// Represents an execution trace of the Mozak VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeCheckRow {
    pub val: u32,
    pub limb_lo: u16,
    pub limb_hi: u16,
    pub filter_cpu: u32,
}

/// Represents an execution trace of the Mozak VM.
#[derive(Debug, Default, Clone)]
pub struct Trace {
    pub rangecheck_column: Vec<RangeCheckRow>,
}
