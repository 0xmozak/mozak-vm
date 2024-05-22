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

    fn compare(&self, rhs: &Self) -> BranchAddressComparison {
        let (parent, child) = match self.height.cmp(&rhs.height) {
            // LHS and RHS are at the same level
            Ordering::Equal => {
                // Check if LHS and RHS have the same parent
                let lhs_msb = self.addr.0 >> 1;
                let rhs_msb = rhs.addr.0 >> 1;
                match lhs_msb.cmp(&rhs_msb) {
                    Ordering::Less => return BranchAddressComparison::RightCousin,
                    Ordering::Equal => {},
                    Ordering::Greater => return BranchAddressComparison::LeftCousin,
                }

                // Compare the final direction of LHS and RHS
                let lhs_lsb = self.addr.0 & 1;
                let rhs_lsb = rhs.addr.0 & 1;
                return match lhs_lsb.cmp(&rhs_lsb) {
                    Ordering::Less => BranchAddressComparison::RightSibling,
                    Ordering::Equal => BranchAddressComparison::Equal,
                    Ordering::Greater => BranchAddressComparison::LeftSibling,
                }
            },
            // LHS is a child of RHS
            Ordering::Less => (rhs, self),
            // RHS is a child of LHS
            Ordering::Greater => (self, rhs),
        };

        let lhs_is_parent = parent == self;

        // Check if child actually descends from parent
        let delta = parent.height - child.height;
        let child_addr = child.addr.0 >> delta;
        match (child_addr.cmp(&parent.addr.0), lhs_is_parent) {
            (Ordering::Less, false) | (Ordering::Greater, true) => return BranchAddressComparison::RightCousin,
            (Ordering::Greater, false) | (Ordering::Less, true) => return BranchAddressComparison::LeftCousin,
            (Ordering::Equal, _) => {},
        }

        let addr = Address(child.addr.0 & ((1 << delta) - 1));
        match (addr.next(delta).1, lhs_is_parent) {
            (Dir::Left, true) => BranchAddressComparison::LeftChild,
            (Dir::Right, true) => BranchAddressComparison::RightChild,
            (Dir::Left, false) => BranchAddressComparison::LeftParent,
            (Dir::Right, false) => BranchAddressComparison::RightParent,
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
    
    /// The RHS address is a cousin somewhere to the left of the LHS address
    LeftCousin,
    /// The RHS address is a cousin somewhere to the right of the LHS address
    RightCousin,
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_branch_compare() {
        let dirs = [Dir::Left, Dir::Right];
        let parent = BranchAddress::root(10);
        let children = dirs.map(|d| parent.child(d).unwrap());
        let grandchildren = children.map(|c| dirs.map(|d| c.child(d).unwrap()));
        let great_grandchildren = grandchildren.map(|c| c.map(|c| dirs.map(|d| c.child(d).unwrap())));

        // Test all self equality
        assert_eq!(parent.compare(&parent), BranchAddressComparison::Equal);
        for c in children {
            assert_eq!(c.compare(&c), BranchAddressComparison::Equal);
        }
        for c in grandchildren.into_iter().flatten() {
            assert_eq!(c.compare(&c), BranchAddressComparison::Equal);
        }
        for c in great_grandchildren.into_iter().flatten().flatten() {
            assert_eq!(c.compare(&c), BranchAddressComparison::Equal);
        }

        // Parent LHS
        assert_eq!(parent.compare(&children[0]), BranchAddressComparison::LeftChild);
        assert_eq!(parent.compare(&children[1]), BranchAddressComparison::RightChild);
        for c in grandchildren[0] {
            assert_eq!(parent.compare(&c), BranchAddressComparison::LeftChild);
        }
        for c in grandchildren[1] {
            assert_eq!(parent.compare(&c), BranchAddressComparison::RightChild);
        }
        for c in great_grandchildren[0].into_iter().flatten() {
            assert_eq!(parent.compare(&c), BranchAddressComparison::LeftChild);
        }
        for c in great_grandchildren[1].into_iter().flatten() {
            assert_eq!(parent.compare(&c), BranchAddressComparison::RightChild);
        }

        // children[0] LHS
        assert_eq!(children[0].compare(&parent), BranchAddressComparison::LeftParent);
        assert_eq!(children[0].compare(&children[1]), BranchAddressComparison::RightSibling);
        assert_eq!(children[0].compare(&grandchildren[0][0]), BranchAddressComparison::LeftChild);
        assert_eq!(children[0].compare(&grandchildren[0][1]), BranchAddressComparison::RightChild);
        for c in grandchildren[1] {
            assert_eq!(children[0].compare(&c), BranchAddressComparison::RightCousin);
        }
        for c in great_grandchildren[0][0] {
            assert_eq!(children[0].compare(&c), BranchAddressComparison::LeftChild);
        }
        for c in great_grandchildren[0][1] {
            assert_eq!(children[0].compare(&c), BranchAddressComparison::RightChild);
        }
        for c in great_grandchildren[1].into_iter().flatten() {
            assert_eq!(children[0].compare(&c), BranchAddressComparison::RightCousin);
        }

        // children[1] LHS
        assert_eq!(children[1].compare(&parent), BranchAddressComparison::RightParent);
        assert_eq!(children[1].compare(&children[0]), BranchAddressComparison::LeftSibling);
        assert_eq!(children[1].compare(&grandchildren[1][0]), BranchAddressComparison::LeftChild);
        assert_eq!(children[1].compare(&grandchildren[1][1]), BranchAddressComparison::RightChild);
        for c in grandchildren[0] {
            assert_eq!(children[1].compare(&c), BranchAddressComparison::LeftCousin);
        }
        for c in great_grandchildren[1][0] {
            assert_eq!(children[1].compare(&c), BranchAddressComparison::LeftChild);
        }
        for c in great_grandchildren[1][1] {
            assert_eq!(children[1].compare(&c), BranchAddressComparison::RightChild);
        }
        for c in great_grandchildren[0].into_iter().flatten() {
            assert_eq!(children[1].compare(&c), BranchAddressComparison::LeftCousin);
        }



        assert_eq!(grandchildren[0][1].compare(&parent), BranchAddressComparison::LeftParent);
        assert_eq!(grandchildren[0][1].compare(&children[1]), BranchAddressComparison::RightCousin);
    }
}
