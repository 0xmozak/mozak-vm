use std::fmt;
use std::ops::{BitAnd, BitAndAssign, Shl, Sub};

pub mod state;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(pub u64);

impl Address {
    fn next(self, height: usize) -> (Option<AddressPath>, Dir) {
        debug_assert!(self.0 < (1 << (height + 1)));
        AddressPath {
            height,
            addr: self.0,
        }
        .next()
    }
}

/// The remaining bits of an address to be consumed as one traverses down the
/// tree towards a leaf.
#[derive(Debug, Clone, Copy)]
pub struct AddressPath<T = u64> {
    /// One less than the number of bits remaining in `addr`
    ///
    /// So `height == 0` means 1 bit remaining, `1` means 2 bits remaining.
    ///
    /// This means that `1 << height` will mask off the MSB.
    height: usize,
    addr: T,
}

impl<T> AddressPath<T>
where
    T: Copy
        + From<bool>
        + Shl<usize, Output = T>
        + BitAnd<T, Output = T>
        + PartialEq
        + fmt::Debug
        + Sub<T, Output = T>
        + BitAndAssign,
{
    pub fn path(addr: T, bits: usize) -> Option<Self> {
        (bits != 0).then_some(Self {
            height: bits - 1,
            addr,
        })
    }

    /// Returns `true` if all remaining directions are `Dir::Left`
    pub fn is_zero(self) -> bool { self.addr == T::from(false) }

    pub fn next(mut self) -> (Option<Self>, Dir) {
        let zero = T::from(false);
        let one = T::from(true);

        // look at the MSB for the current direction
        let msb_mask = one << self.height;

        let dir = if self.addr & msb_mask == zero {
            Dir::Left
        } else {
            Dir::Right
        };

        // Pop the MSB
        self.addr &= msb_mask - one;

        if self.height == 0 {
            debug_assert_eq!(self.addr, zero);
            (None, dir)
        } else {
            self.height -= 1;
            (Some(self), dir)
        }
    }
}

/// A partial address which is constructed starting at the root and moving
/// downward, adding on one bit at a time based on a provided direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchAddress {
    height: usize,
    addr: Address,
}

impl BranchAddress {
    /// Initialize the `BranchAddress` to the root node
    #[must_use]
    pub fn root(height: usize) -> Self {
        Self {
            height,
            addr: Address(0),
        }
    }

    /// Move downward, adding a `0|1` bit based on the dir (`Left|Right`).
    ///
    /// # Errors
    ///
    /// If we've reached the bottom, return a `Err(Address)` instead
    pub fn child(mut self, dir: Dir) -> Result<Self, Address> {
        self.addr.0 <<= 1;
        self.addr.0 |= u64::from(dir == Dir::Right);
        if self.height == 0 {
            Err(self.addr)
        } else {
            self.height -= 1;
            Ok(self)
        }
    }
}
