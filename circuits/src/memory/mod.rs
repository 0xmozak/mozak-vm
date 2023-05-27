extern crate alloc;

use alloc::collections::BTreeMap;
use field::BaseField;
use field::FieldElement as Fp;

pub mod columns;
pub mod air;

#[derive(Copy, Clone)]
pub enum Operation<F: BaseField> {
    Read(F, F),
    Write(F, F),
}

impl<F: BaseField> Operation<F> {
    pub fn get_address(&self) -> F {
        match self {
            Operation::Read(addr, _) => *addr,
            Operation::Write(addr, _) => *addr,
        }
    }
    pub fn get_value(&self) -> F {
        match self {
            Operation::Read(_, value) => *value,
            Operation::Write(_, value) => *value,
        }
    }
}

#[derive(Default)]
pub struct MemoryChip<F: BaseField> {
    pub cells: BTreeMap<F, F>,
    pub operations: BTreeMap<F, Vec<Operation<F>>>,
}

impl<F: BaseField> MemoryChip<F> {
    pub fn new() -> Self {
        Self {
            cells: BTreeMap::new(),
            operations: BTreeMap::new(),
        }
    }

    pub fn read(&mut self, clk: F, address: F, log: bool) -> F {
        let value = self.cells.get(&address.into()).copied().unwrap();
        if log {
            self.operations
                .entry(clk)
                .or_insert_with(Vec::new)
                .push(Operation::Read(address.into(), value));
        }
        value
    }

    pub fn write(&mut self, clk: F, address: F, value: F, log: bool) {
        if log {
            self.operations
                .entry(clk)
                .or_insert_with(Vec::new)
                .push(Operation::Write(address, value));
        }
        self.cells.insert(address, value.into());
    }
}
