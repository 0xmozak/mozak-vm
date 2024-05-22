pub mod state;

#[derive(Debug, Clone, Copy)]
pub struct Address(pub u64);

impl Address {
    fn next(self, height: usize) -> (Option<AddressPath>, Dir) {
        debug_assert!(self.0 <= (1 << height));
        AddressPath { height, addr: self }.next()
    }
}

/// The remaining bits of an address to be consumed as one traverses down the
/// tree towards a leaf.
#[derive(Debug, Clone, Copy)]
struct AddressPath {
    height: usize,
    addr: Address,
}

impl AddressPath {
    fn next(mut self) -> (Option<Self>, Dir) {
        // look at the MSB for the current direction
        let bit = 1 << self.height;

        let dir = if self.addr.0 & bit != 0 {
            Dir::Right
        } else {
            Dir::Left
        };

        // Pop the MSB
        self.addr.0 &= bit - 1;

        if self.height == 0 {
            debug_assert_eq!(self.addr.0, 0);
            (None, dir)
        } else {
            self.height -= 1;
            (Some(self), dir)
        }
    }
}

/// A partial address which is constructed starting at the root and moving
/// downward, adding on one bit at a time based on a provided direction
#[derive(Debug, Clone, Copy)]
struct BranchAddress {
    height: usize,
    addr: Address,
}

impl BranchAddress {
    /// Initialize the `BranchAddress` to the root node
    fn root(height: usize) -> Self {
        Self {
            height,
            addr: Address(0),
        }
    }

    /// Move downward, adding a `0|1` bit based on the dir (`Left|Right`).
    /// If we've reached the bottom, return an `Address` instead
    fn child(mut self, dir: Dir) -> Result<Self, Address> {
        self.addr = Address(
            self.addr.0 << 1
                | match dir {
                    Dir::Left => 0,
                    Dir::Right => 1,
                },
        );
        if self.height == 0 {
            Err(self.addr)
        } else {
            self.height -= 1;
            Ok(self)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Left,
    Right,
}
