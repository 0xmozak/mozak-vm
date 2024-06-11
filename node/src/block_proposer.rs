use std::cmp::Ordering;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, Shl, Sub};

use itertools::Itertools;
use mozak_recproofs::{Event, EventType as ProofEventType};
use mozak_sdk::common::types::{
    CanonicalEvent, EventType as SdkEventType, ProgramIdentifier, StateAddress,
};
use plonky2::field::types::Field;

use crate::F;

pub mod matches;
pub mod state;
pub mod transactions;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct OngoingTxKey {
    cast_root: [F; 4],
    call_tape: [F; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
}

#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address(pub u64);

impl Address {
    #[must_use]
    pub const fn from_state(v: StateAddress) -> Self { Self(u64::from_le_bytes(v.0)) }

    #[must_use]
    pub const fn to_state(self) -> StateAddress { StateAddress(self.0.to_le_bytes()) }

    #[must_use]
    fn next(self, height: usize) -> (Option<AddressPath>, Dir) {
        debug_assert!(u128::from(self.0) < (1 << (height + 1)));
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

impl PartialOrd<BranchAddress> for BranchAddress {
    fn partial_cmp(&self, other: &BranchAddress) -> Option<Ordering> {
        if self.height == other.height {
            Some(Ord::cmp(&self.addr.0, &other.addr.0))
        } else {
            None
        }
    }
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

    /// Initialize the `BranchAddress` to a leaf node
    #[must_use]
    pub fn base(a: u64) -> Self {
        BranchAddress {
            height: 0,
            addr: Address(a),
        }
    }

    /// Find the common ancestor between `self` and `rhs`
    #[must_use]
    pub fn common_ancestor(mut self, mut rhs: Self) -> Self {
        // Get both to the same height
        let d1 = self.height.saturating_sub(rhs.height);
        let d2 = rhs.height.saturating_sub(self.height);
        self = self.parent(d2);
        rhs = rhs.parent(d1);

        // Find where the two diverge by XORing and then taking the MSB position
        let ancestor_diff = u64::BITS - (self.addr.0 ^ rhs.addr.0).leading_zeros();
        self = self.parent(ancestor_diff as usize);
        debug_assert_eq!(self, rhs.parent(ancestor_diff as usize));
        self
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

    /// Move upward, removing the bottom `n` bits.
    /// If we've reached the bottom, return an `Address` instead
    #[must_use]
    pub fn parent(mut self, n: usize) -> Self {
        self.addr.0 >>= n;
        self.height += n;
        self
    }

    #[must_use]
    pub fn compare(self, rhs: Self) -> BranchAddressComparison {
        let (parent, child) = match self.height.cmp(&rhs.height) {
            // LHS and RHS are at the same level
            Ordering::Equal => {
                // Check if LHS and RHS have the same parent
                let lhs_msb = self.addr.0 >> 1;
                let rhs_msb = rhs.addr.0 >> 1;
                match lhs_msb.cmp(&rhs_msb) {
                    Ordering::Less => return BranchAddressComparison::RightCousin,
                    Ordering::Equal => {}
                    Ordering::Greater => return BranchAddressComparison::LeftCousin,
                }

                // Compare the final direction of LHS and RHS
                let lhs_lsb = self.addr.0 & 1;
                let rhs_lsb = rhs.addr.0 & 1;
                return match lhs_lsb.cmp(&rhs_lsb) {
                    Ordering::Less => BranchAddressComparison::RightSibling,
                    Ordering::Equal => BranchAddressComparison::Equal,
                    Ordering::Greater => BranchAddressComparison::LeftSibling,
                };
            }
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
            (Ordering::Less, false) | (Ordering::Greater, true) =>
                return BranchAddressComparison::RightCousin,
            (Ordering::Greater, false) | (Ordering::Less, true) =>
                return BranchAddressComparison::LeftCousin,
            (Ordering::Equal, _) => {}
        }

        let addr = Address(child.addr.0 & ((1 << delta) - 1));
        match (addr.next(delta - 1).1, lhs_is_parent) {
            (Dir::Left, true) => BranchAddressComparison::LeftChild,
            (Dir::Right, true) => BranchAddressComparison::RightChild,
            (Dir::Left, false) => BranchAddressComparison::LeftParent,
            (Dir::Right, false) => BranchAddressComparison::RightParent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchAddressComparison {
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

/// Convert the sdk enum to the recproof enum
#[must_use]
pub fn convert_event_type(ty: SdkEventType) -> ProofEventType {
    match ty {
        SdkEventType::Write => ProofEventType::Write,
        SdkEventType::Ensure => ProofEventType::Ensure,
        SdkEventType::Read => ProofEventType::Read,
        SdkEventType::GiveOwner => ProofEventType::GiveOwner,
        SdkEventType::TakeOwner => ProofEventType::TakeOwner,
    }
}

/// Convert an sdk event to a recproof event
#[must_use]
pub fn convert_event(id: &ProgramIdentifier, e: &CanonicalEvent) -> Event<F> {
    Event {
        owner: id.0.to_u64s().map(F::from_noncanonical_u64),
        ty: convert_event_type(e.type_),
        address: u64::from_le_bytes(e.address.0),
        value: e.value.to_u64s().map(F::from_noncanonical_u64),
    }
}

/// Reduces a tree by merging all the items, grouped by their address,
/// then reducing their addresses
#[allow(clippy::missing_panics_doc)]
pub fn reduce_tree_by_address<A: Clone + PartialEq, T>(
    mut iter: Vec<(A, T)>,
    mut addr_inc: impl FnMut(A) -> A,
    mut merge: impl FnMut(&A, T, T) -> T,
) -> Option<(A, T)> {
    while iter.len() > 1 {
        iter = reduce_tree_by_address_step(iter, &mut addr_inc, &mut merge).collect();
    }
    iter.pop()
}

/// Reduces a tree by merging all the items, grouped by their address,
/// then reducing their addresses
#[allow(clippy::missing_panics_doc)]
pub fn reduce_tree_by_address_step<A: Clone + PartialEq, T>(
    iter: impl IntoIterator<Item = (A, T)>,
    mut addr_inc: impl FnMut(A) -> A,
    mut merge: impl FnMut(&A, T, T) -> T,
) -> impl Iterator<Item = (A, T)> {
    let chunks = iter.into_iter().chunk_by(|e| e.0.clone());

    std::iter::from_fn(move || {
        chunks
            .into_iter()
            .map(|(address, ts)| {
                let ts = ts.map(|x| x.1);
                let t = reduce_tree(ts, |x| x, |x| x, |l, r| merge(&address, l, r)).unwrap();
                (addr_inc(address), t)
            })
            .next()
    })
}

/// Reduces a tree by merging all the items
#[must_use]
pub fn reduce_tree<T, R>(
    iter: impl IntoIterator<Item = T>,
    make_ret: impl FnOnce(T) -> R,
    mut make_t: impl FnMut(R) -> T,
    mut merge: impl FnMut(T, T) -> R,
) -> Option<R> {
    let mut i = iter.into_iter();

    let cap = if i.size_hint().0 == 0 {
        0
    } else {
        i.size_hint().0.ilog2() as usize + 1
    };

    let mut stack: Vec<(R, usize)> = Vec::with_capacity(cap);
    let final_v = loop {
        let Some(v0) = i.next() else {
            break None;
        };
        let Some(v1) = i.next() else {
            break Some(v0);
        };
        let (mut v, mut c) = (merge(v0, v1), 2);

        while let Some((pv, pc)) = stack.pop() {
            if pc != c {
                stack.push((pv, pc));
                break;
            }
            v = merge(make_t(pv), make_t(v));
            c += pc;
        }
        stack.push((v, c));
    };

    let mut v = match (stack.pop(), final_v) {
        (None, None) => return None,
        (Some((pv, _)), None) => pv,
        (None, Some(v)) => return Some(make_ret(v)),
        (Some((pv, _)), Some(v)) => merge(make_t(pv), v),
    };
    while let Some((pv, _)) = stack.pop() {
        v = merge(make_t(pv), make_t(v));
    }
    Some(v)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_common_ancestor() {
        let dirs = [Dir::Left, Dir::Right];
        let parent = BranchAddress::root(10);
        let children = dirs.map(|d| parent.child(d).unwrap());
        let grandchildren = children.map(|c| dirs.map(|d| c.child(d).unwrap()));
        let great_grandchildren =
            grandchildren.map(|c| c.map(|c| dirs.map(|d| c.child(d).unwrap())));

        let a = great_grandchildren[0][0][0];
        let b = great_grandchildren[0][0][0];
        let c = great_grandchildren[0][0][0];
        assert_eq!(a.common_ancestor(b), c);
        assert_eq!(b.common_ancestor(a), c);

        let a = great_grandchildren[0][0][0];
        let b = great_grandchildren[0][0][1];
        let c = grandchildren[0][0];
        assert_eq!(a.common_ancestor(b), c);
        assert_eq!(b.common_ancestor(a), c);

        let a = great_grandchildren[0][0][0];
        let b = great_grandchildren[0][1][1];
        let c = children[0];
        assert_eq!(a.common_ancestor(b), c);
        assert_eq!(b.common_ancestor(a), c);

        let a = great_grandchildren[0][0][0];
        let b = great_grandchildren[1][0][0];
        let c = parent;
        assert_eq!(a.common_ancestor(b), c);
        assert_eq!(b.common_ancestor(a), c);
    }

    #[test]
    fn test_branch_compare() {
        let dirs = [Dir::Left, Dir::Right];
        let parent = BranchAddress::root(10);
        let children = dirs.map(|d| parent.child(d).unwrap());
        let grandchildren = children.map(|c| dirs.map(|d| c.child(d).unwrap()));
        let great_grandchildren =
            grandchildren.map(|c| c.map(|c| dirs.map(|d| c.child(d).unwrap())));

        // Test all self equality
        assert_eq!(parent.compare(parent), BranchAddressComparison::Equal);
        for c in children {
            assert_eq!(c.compare(c), BranchAddressComparison::Equal);
        }
        for c in grandchildren.into_iter().flatten() {
            assert_eq!(c.compare(c), BranchAddressComparison::Equal);
        }
        for c in great_grandchildren.into_iter().flatten().flatten() {
            assert_eq!(c.compare(c), BranchAddressComparison::Equal);
        }

        // Parent LHS
        assert_eq!(
            parent.compare(children[0]),
            BranchAddressComparison::LeftChild
        );
        assert_eq!(
            parent.compare(children[1]),
            BranchAddressComparison::RightChild
        );
        for c in grandchildren[0] {
            assert_eq!(parent.compare(c), BranchAddressComparison::LeftChild);
        }
        for c in grandchildren[1] {
            assert_eq!(parent.compare(c), BranchAddressComparison::RightChild);
        }
        for c in great_grandchildren[0].into_iter().flatten() {
            assert_eq!(parent.compare(c), BranchAddressComparison::LeftChild);
        }
        for c in great_grandchildren[1].into_iter().flatten() {
            assert_eq!(parent.compare(c), BranchAddressComparison::RightChild);
        }

        // children[0] LHS
        assert_eq!(
            children[0].compare(parent),
            BranchAddressComparison::LeftParent
        );
        assert_eq!(
            children[0].compare(children[1]),
            BranchAddressComparison::RightSibling
        );
        assert_eq!(
            children[0].compare(grandchildren[0][0]),
            BranchAddressComparison::LeftChild
        );
        assert_eq!(
            children[0].compare(grandchildren[0][1]),
            BranchAddressComparison::RightChild
        );
        for c in grandchildren[1] {
            assert_eq!(children[0].compare(c), BranchAddressComparison::RightCousin);
        }
        for c in great_grandchildren[0][0] {
            assert_eq!(children[0].compare(c), BranchAddressComparison::LeftChild);
        }
        for c in great_grandchildren[0][1] {
            assert_eq!(children[0].compare(c), BranchAddressComparison::RightChild);
        }
        for c in great_grandchildren[1].into_iter().flatten() {
            assert_eq!(children[0].compare(c), BranchAddressComparison::RightCousin);
        }

        // children[1] LHS
        assert_eq!(
            children[1].compare(parent),
            BranchAddressComparison::RightParent
        );
        assert_eq!(
            children[1].compare(children[0]),
            BranchAddressComparison::LeftSibling
        );
        assert_eq!(
            children[1].compare(grandchildren[1][0]),
            BranchAddressComparison::LeftChild
        );
        assert_eq!(
            children[1].compare(grandchildren[1][1]),
            BranchAddressComparison::RightChild
        );
        for c in grandchildren[0] {
            assert_eq!(children[1].compare(c), BranchAddressComparison::LeftCousin);
        }
        for c in great_grandchildren[1][0] {
            assert_eq!(children[1].compare(c), BranchAddressComparison::LeftChild);
        }
        for c in great_grandchildren[1][1] {
            assert_eq!(children[1].compare(c), BranchAddressComparison::RightChild);
        }
        for c in great_grandchildren[0].into_iter().flatten() {
            assert_eq!(children[1].compare(c), BranchAddressComparison::LeftCousin);
        }

        assert_eq!(
            grandchildren[0][1].compare(parent),
            BranchAddressComparison::LeftParent
        );
        assert_eq!(
            grandchildren[0][1].compare(children[1]),
            BranchAddressComparison::RightCousin
        );
    }
}
