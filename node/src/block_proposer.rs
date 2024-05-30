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
pub struct AddressPath {
    /// One less than the number of bits remaining in `addr`
    ///
    /// So `height == 0` means 1 bit remaining, `1` means 2 bits remaining.
    ///
    /// This means that `1 << height` will mask off the MSB.
    height: usize,
    addr: u64,
}

impl AddressPath {
    fn next(mut self) -> (Option<Self>, Dir) {
        // look at the MSB for the current direction
        let msb_mask = 1 << self.height;

        let dir = if self.addr & msb_mask != 0 {
            Dir::Right
        } else {
            Dir::Left
        };

        // Pop the MSB
        self.addr &= msb_mask - 1;

        if self.height == 0 {
            debug_assert_eq!(self.addr, 0);
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
