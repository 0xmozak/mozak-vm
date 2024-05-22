use std::cmp::Ordering;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, Shl, Sub};

pub mod state;
pub mod transactions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(pub u64);

impl Address {
    fn next(self, height: usize) -> (Option<AddressPath>, Dir) {
        debug_assert!(self.0 < (1 << height));
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
struct AddressPath<T = u64> {
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
    fn path(addr: T, height: usize) -> Option<Self> {
        (height == 0).then_some(Self { height, addr })
    }

    fn next(mut self) -> (Option<Self>, Dir) {
        let zero = T::from(false);
        let one = T::from(true);

        // look at the MSB for the current direction
        let bit = one << (self.height - 1);

        let dir = if self.addr & bit != zero {
            Dir::Right
        } else {
            Dir::Left
        };

        // Pop the MSB
        self.addr &= bit - one;

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
    fn root(height: usize) -> Self {
        Self {
            height,
            addr: Address(0),
        }
    }

    /// Initialize the `BranchAddress` to a leaf node
    fn base(a: u64) -> Self {
        BranchAddress {
            height: 0,
            addr: Address(a),
        }
    }

    /// Move downward, adding a `0|1` bit based on the dir (`Left|Right`).
    /// If we've reached the bottom, return an `Address` instead
    fn child(mut self, dir: Dir) -> Result<Self, Address> {
        self.addr.0 <<= 1;
        self.addr.0 |= (dir == Dir::Right) as u64;
        if self.height == 0 {
            Err(self.addr)
        } else {
            self.height -= 1;
            Ok(self)
        }
    }

    /// Move upward, adding a `0|1` bit based on the dir (`Left|Right`).
    /// If we've reached the bottom, return an `Address` instead
    fn parent(mut self) -> Self {
        self.addr = Address(self.addr.0 >> 1);
        self.height += 1;
        self
    }

    fn compare(&self, rhs: &Self) -> Option<BranchAddressComparison> {
        let (parent, child) = match self.height.cmp(&rhs.height) {
            // LHS and RHS are at the same level
            Ordering::Equal => {
                // Check if LHS and RHS have the same parent
                let lhs_msb = self.addr.0 >> 1;
                let rhs_msb = rhs.addr.0 >> 1;
                if lhs_msb != rhs_msb {
                    return None
                }

                // Compare the final direction of LHS and RHS
                let lhs_lsb = self.addr.0 & 1;
                let rhs_lsb = rhs.addr.0 & 1;
                return match lhs_lsb.cmp(&rhs_lsb) {
                    Ordering::Less => Some(BranchAddressComparison::RightSibling),
                    Ordering::Equal => Some(BranchAddressComparison::Equal),
                    Ordering::Greater => Some(BranchAddressComparison::LeftSibling),
                }
            },
            // LHS is a child of RHS
            Ordering::Less => (rhs, self),
            // RHS is a child of LHS
            Ordering::Greater => (self, rhs),
        };


        // Check if child actually descends from parent
        let delta = parent.height - child.height;
        if child.addr.0 >> delta != parent.addr.0 {
            return None
        }

        let addr = Address(child.addr.0 & ((1 << delta) - 1));
        match (addr.next(delta).1, parent == self) {
            (Dir::Left, true) => Some(BranchAddressComparison::LeftChild),
            (Dir::Right, true) => Some(BranchAddressComparison::RightChild),
            (Dir::Left, false) => Some(BranchAddressComparison::LeftParent),
            (Dir::Right, false) => Some(BranchAddressComparison::RightParent),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchAddressComparison {
    /// The LHS and RHS addresses are the same
    Equal,
    /// The RHS address is a left-child of the LHS address
    LeftChild,
    /// The RHS address is a right-child of the LHS address
    RightChild,
    /// The RHS address is a left-sibling of the LHS address
    LeftSibling,
    /// The RHS address is a right-sibling of the LHS address
    RightSibling,
    /// The RHS address is a left-parent of the LHS address
    LeftParent,
    /// The RHS address is a right-parent of the LHS address
    RightParent,
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_branch_compare() {
        let parent = BranchAddress::root(10);
        let child_1 = parent.child(Dir::Left).unwrap();
        let child_2 = parent.child(Dir::Right).unwrap();
        let child_3 = child_1.child(Dir::Right).unwrap();

        assert_eq!(parent.compare(&parent), Some(BranchAddressComparison::Equal));
        assert_eq!(child_1.compare(&child_1), Some(BranchAddressComparison::Equal));
        assert_eq!(child_2.compare(&child_2), Some(BranchAddressComparison::Equal));
        assert_eq!(child_3.compare(&child_3), Some(BranchAddressComparison::Equal));

        assert_eq!(parent.compare(&child_1), Some(BranchAddressComparison::LeftChild));
        assert_eq!(child_1.compare(&parent), Some(BranchAddressComparison::LeftParent));
        assert_eq!(parent.compare(&child_3), Some(BranchAddressComparison::LeftChild));
        assert_eq!(child_3.compare(&parent), Some(BranchAddressComparison::LeftParent));

        assert_eq!(child_1.compare(&child_2), Some(BranchAddressComparison::RightSibling));
        assert_eq!(child_2.compare(&child_1), Some(BranchAddressComparison::LeftSibling));

        assert_eq!(parent.compare(&child_2), Some(BranchAddressComparison::RightChild));
        assert_eq!(child_2.compare(&parent), Some(BranchAddressComparison::RightParent));

        assert_eq!(child_2.compare(&child_3), None);
        assert_eq!(child_3.compare(&child_2), None);
    }
}
