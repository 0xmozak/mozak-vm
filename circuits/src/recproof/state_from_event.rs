use enumflags2::{bitflags, BitFlags};
use itertools::{chain, Itertools};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use super::maybe_connect;

// Limit transfers to 2^40 credits to avoid overflow issues
const MAX_LEAF_TRANSFER: usize = 40;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EventType {
    Write = 0,
    Ensure = 1,
    Read = 2,
    GiveOwner = 3,
    TakeOwner = 4,
    CreditDelta = 5,
}

impl EventType {
    fn constant<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> Target
    where
        F: RichField + Extendable<D>, {
        builder.constant(F::from_canonical_u64(self as u64))
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Flags {
    WriteFlag = 1 << 0,
    EnsureFlag = 1 << 1,
    ReadFlag = 1 << 2,
    GiveOwnerFlag = 1 << 3,
    TakeOwnerFlag = 1 << 4,
}

impl Flags {
    fn count() -> usize { BitFlags::<Self>::ALL.len() }

    fn index(self) -> usize { (self as u8).trailing_zeros() as usize }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PublicIndices {
    /// The index of the event/object address
    pub address: usize,

    /// The index of the (partial) object flags
    pub object_flags: usize,

    /// The indices of each of the elements of the previous constraint owner
    pub old_owner: [usize; 4],

    /// The indices of each of the elements of the new constraint owner
    pub new_owner: [usize; 4],

    /// The indices of each of the elements of the previous data
    pub old_data: [usize; 4],

    /// The indices of each of the elements of the new data
    pub new_data: [usize; 4],

    /// The index of the credit delta
    pub credit_delta: usize,
}

impl PublicIndices {
    /// Extract `address` from an array of public inputs.
    pub fn get_address<T: Copy>(&self, public_inputs: &[T]) -> T { public_inputs[self.address] }

    /// Insert `address` into an array of public inputs.
    pub fn set_address<T>(&self, public_inputs: &mut [T], v: T) { public_inputs[self.address] = v; }

    /// Extract `object_flags` from an array of public inputs.
    pub fn get_object_flags<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.object_flags]
    }

    /// Insert `object_flags` into an array of public inputs.
    pub fn set_object_flags<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.object_flags] = v;
    }

    /// Extract `old_owner` from an array of public inputs.
    pub fn get_old_owner<T: Copy>(&self, public_inputs: &[T]) -> [T; 4] {
        self.old_owner.map(|i| public_inputs[i])
    }

    /// Insert `old_owner` into an array of public inputs.
    pub fn set_old_owner<T>(&self, public_inputs: &mut [T], v: [T; 4]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.old_owner[i]] = v;
        }
    }

    /// Extract `new_owner` from an array of public inputs.
    pub fn get_new_owner<T: Copy>(&self, public_inputs: &[T]) -> [T; 4] {
        self.new_owner.map(|i| public_inputs[i])
    }

    /// Insert `new_owner` into an array of public inputs.
    pub fn set_new_owner<T>(&self, public_inputs: &mut [T], v: [T; 4]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.new_owner[i]] = v;
        }
    }

    /// Extract `old_data` from an array of public inputs.
    pub fn get_old_data<T: Copy>(&self, public_inputs: &[T]) -> [T; 4] {
        self.old_data.map(|i| public_inputs[i])
    }

    /// Insert `old_data` into an array of public inputs.
    pub fn set_old_data<T>(&self, public_inputs: &mut [T], v: [T; 4]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.old_data[i]] = v;
        }
    }

    /// Extract `new_data` from an array of public inputs.
    pub fn get_new_data<T: Copy>(&self, public_inputs: &[T]) -> [T; 4] {
        self.new_data.map(|i| public_inputs[i])
    }

    /// Insert `new_data` into an array of public inputs.
    pub fn set_new_data<T>(&self, public_inputs: &mut [T], v: [T; 4]) {
        for (i, v) in v.into_iter().enumerate() {
            public_inputs[self.new_data[i]] = v;
        }
    }

    /// Extract `credit_delta` from an array of public inputs.
    pub fn get_credit_delta<T: Copy>(&self, public_inputs: &[T]) -> T {
        public_inputs[self.credit_delta]
    }

    /// Insert `credit_delta` into an array of public inputs.
    pub fn set_credit_delta<T>(&self, public_inputs: &mut [T], v: T) {
        public_inputs[self.credit_delta] = v;
    }
}

pub struct SubCircuitInputs {
    /// The event/object address
    pub address: Target,

    /// The (partial) object flags
    pub object_flags: Target,

    /// The previous constraint owner
    pub old_owner: [Target; 4],

    /// The new constraint owner
    pub new_owner: [Target; 4],

    /// The previous data
    pub old_data: [Target; 4],

    /// The new data
    pub new_data: [Target; 4],

    /// The credit delta
    pub credit_delta: Target,
}

pub struct LeafTargets {
    /// The public inputs
    pub inputs: SubCircuitInputs,

    /// The originator of the event
    pub event_owner: [Target; 4],

    /// The event type
    pub event_ty: Target,

    /// The event value. Has different meanings for each event.
    ///
    /// For `Write` and `Ensure`, this is the `new_data` value.
    /// For `Read` this is the `old_data` value.
    /// For `GiveOwner` this is the `new_owner` value.
    /// For `TakeOwner` this is the `old_owner` value.
    /// For `CreditDelta` this is the operation (addition or subtraction) and
    /// the amount of credits to add/subtract. The format is `[d, _, _, s]`
    /// where `d` is the delta, and `s == 0` means add and `s == -1` means
    /// subtract.
    pub event_value: [Target; 4],
}

struct SplitFlags {
    new_data: Target,
    owner: Target,

    write: BoolTarget,
    ensure: BoolTarget,
    read: BoolTarget,
    give_owner: BoolTarget,
    take_owner: BoolTarget,
}

impl SplitFlags {
    fn split<F, const D: usize>(builder: &mut CircuitBuilder<F, D>, flags: Target) -> Self
    where
        F: RichField + Extendable<D>, {
        let new_data_flag_count = (Flags::WriteFlag | Flags::EnsureFlag).len();
        let other_flag_count = Flags::count() - new_data_flag_count;
        let owner_flag_count = (Flags::GiveOwnerFlag | Flags::TakeOwnerFlag).len();

        // Split off the flags corresponding to changes to new_data
        let (new_data, flags) = builder.split_low_high(flags, new_data_flag_count, Flags::count());
        // Split  the flag corresponding to read from the owner flags
        let (read, owner) = builder.split_low_high(flags, 1, other_flag_count);
        let read = BoolTarget::new_unsafe(read);

        let new_data_flags = builder.split_le(new_data, new_data_flag_count);
        let owner_flags = builder.split_le(owner, owner_flag_count);

        let flags = chain!(new_data_flags, [read], owner_flags).collect_vec();

        Self {
            new_data,
            owner,
            write: flags[Flags::WriteFlag.index()],
            ensure: flags[Flags::EnsureFlag.index()],
            read: flags[Flags::ReadFlag.index()],
            give_owner: flags[Flags::GiveOwnerFlag.index()],
            take_owner: flags[Flags::TakeOwnerFlag.index()],
        }
    }
}

impl SubCircuitInputs {
    #[must_use]
    pub fn default<F, const D: usize>(builder: &mut CircuitBuilder<F, D>) -> Self
    where
        F: RichField + Extendable<D>, {
        let address = builder.add_virtual_target();
        let object_flags = builder.add_virtual_target();
        let old_owner = builder.add_virtual_target_arr();
        let new_owner = builder.add_virtual_target_arr();
        let old_data = builder.add_virtual_target_arr();
        let new_data = builder.add_virtual_target_arr();
        let credit_delta = builder.add_virtual_target();

        builder.register_public_input(address);
        builder.register_public_input(object_flags);
        builder.register_public_inputs(&old_owner);
        builder.register_public_inputs(&new_owner);
        builder.register_public_inputs(&old_data);
        builder.register_public_inputs(&new_data);
        builder.register_public_input(credit_delta);

        Self {
            address,
            object_flags,
            old_owner,
            new_owner,
            old_data,
            new_data,
            credit_delta,
        }
    }

    #[must_use]
    pub fn build_leaf<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> LeafTargets
    where
        F: RichField + Extendable<D>, {
        let zero = builder.zero();
        let one = builder.one();
        let write_const = EventType::Write.constant(builder);
        let read_const = EventType::Read.constant(builder);
        let ensure_const = EventType::Ensure.constant(builder);
        let give_owner_const = EventType::GiveOwner.constant(builder);
        let take_owner_const = EventType::TakeOwner.constant(builder);
        let credit_delta_const = EventType::CreditDelta.constant(builder);

        let event_owner = builder.add_virtual_target_arr();
        let event_ty = builder.add_virtual_target();
        let event_value = builder.add_virtual_target_arr();

        let (write_flag, ensure_flag, read_flag, give_owner_flag, take_owner_flag) = {
            let object_flags = builder.split_le(self.object_flags, Flags::count());

            (
                object_flags[Flags::WriteFlag.index()],
                object_flags[Flags::EnsureFlag.index()],
                object_flags[Flags::ReadFlag.index()],
                object_flags[Flags::GiveOwnerFlag.index()],
                object_flags[Flags::TakeOwnerFlag.index()],
            )
        };

        let is_write = builder.is_equal(event_ty, write_const);
        let is_read = builder.is_equal(event_ty, read_const);
        let is_ensure = builder.is_equal(event_ty, ensure_const);
        let is_give_owner = builder.is_equal(event_ty, give_owner_const);
        let is_take_owner = builder.is_equal(event_ty, take_owner_const);
        let is_credit_delta = builder.is_equal(event_ty, credit_delta_const);

        // new data comes from the event value for writes and ensures
        // These are all mutually exclusive (since `event_ty` can't simultaneously equal
        // multiple values), so we can just add them
        let new_data_from_value = builder.add(is_write.target, is_ensure.target);
        let new_data_from_value = BoolTarget::new_unsafe(new_data_from_value);

        // old owner comes from the event owner for writes, give owners, and credit
        // deltas These are all mutually exclusive, so we can just add them
        let old_owner_from_event = builder.add(is_write.target, is_give_owner.target);
        let old_owner_from_event = builder.add(old_owner_from_event, is_credit_delta.target);
        let old_owner_from_event = BoolTarget::new_unsafe(old_owner_from_event);

        // Handle flags
        builder.connect(is_write.target, write_flag.target);
        builder.connect(is_read.target, read_flag.target);
        builder.connect(is_ensure.target, ensure_flag.target);
        builder.connect(is_give_owner.target, give_owner_flag.target);
        builder.connect(is_take_owner.target, take_owner_flag.target);

        // Handle old owner from event owner (write or give)
        maybe_connect(builder, self.old_owner, old_owner_from_event, event_owner);

        // Handle old owner from event value (take)
        maybe_connect(builder, self.old_owner, is_take_owner, event_value);

        // Handle new owner from event owner (take)
        maybe_connect(builder, self.new_owner, is_take_owner, event_owner);

        // Handle new owner from event value (give)
        maybe_connect(builder, self.new_owner, is_give_owner, event_value);

        // Handle old data from event value (read)
        maybe_connect(builder, self.old_data, is_read, event_value);

        // Handle new data from event value (write or ensure)
        maybe_connect(builder, self.new_data, new_data_from_value, event_value);

        // Handle credit delta
        let credit_delta_val = builder.select(is_credit_delta, event_value[0], zero);
        builder.range_check(credit_delta_val, MAX_LEAF_TRANSFER);
        let credit_delta_sign = builder.select(is_credit_delta, event_value[3], zero);
        let credit_delta_sign = builder.mul_const_add(-F::TWO, credit_delta_sign, one);
        let credit_delta_calc = builder.mul(credit_delta_val, credit_delta_sign);
        builder.connect(credit_delta_calc, self.credit_delta);

        LeafTargets {
            inputs: self,

            event_owner,
            event_ty,
            event_value,
        }
    }
}

/// The leaf subcircuit metadata. This subcircuit validates a (public) partial
/// object corresponds to a given (private) event.
pub struct LeafSubCircuit {
    pub targets: LeafTargets,
    pub indices: PublicIndices,
}

impl LeafTargets {
    #[must_use]
    pub fn build(self, public_inputs: &[Target]) -> LeafSubCircuit {
        // Find the indicies
        let indices = PublicIndices {
            address: public_inputs
                .iter()
                .position(|&pi| pi == self.inputs.address)
                .expect("target not found"),
            object_flags: public_inputs
                .iter()
                .position(|&pi| pi == self.inputs.object_flags)
                .expect("target not found"),
            old_owner: self.inputs.old_owner.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            new_owner: self.inputs.new_owner.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            old_data: self.inputs.old_data.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            new_data: self.inputs.new_data.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            credit_delta: public_inputs
                .iter()
                .position(|&pi| pi == self.inputs.credit_delta)
                .expect("target not found"),
        };
        LeafSubCircuit {
            targets: self,
            indices,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LeafWitnessValue<F> {
    pub address: u64,
    pub object_flags: BitFlags<Flags>,
    pub old_owner: [F; 4],
    pub new_owner: [F; 4],
    pub old_data: [F; 4],
    pub new_data: [F; 4],
    pub credit_delta: i64,
    pub event_owner: [F; 4],
    pub event_ty: F,
    pub event_value: [F; 4],
}

impl<F: RichField> LeafWitnessValue<F> {
    pub fn from_event(
        address: u64,
        event_owner: [F; 4],
        event_ty: EventType,
        event_value: [F; 4],
    ) -> Self {
        let zero = [F::ZERO; 4];

        let (object_flags, credit_delta) = match event_ty {
            EventType::Write => (Flags::WriteFlag.into(), 0),
            EventType::Read => (Flags::ReadFlag.into(), 0),
            EventType::Ensure => (Flags::EnsureFlag.into(), 0),
            EventType::GiveOwner => (Flags::GiveOwnerFlag.into(), 0),
            EventType::TakeOwner => (Flags::TakeOwnerFlag.into(), 0),
            EventType::CreditDelta => {
                #[allow(clippy::cast_possible_wrap)]
                let value = event_value[0].to_canonical_u64() as i64;
                let value = match event_value[3] {
                    sign if sign.is_zero() => value,
                    sign if sign.is_one() => -value,
                    _ => unreachable!(),
                };
                (BitFlags::EMPTY, value)
            }
        };

        let (old_owner, new_owner) = match event_ty {
            EventType::Write | EventType::CreditDelta => (event_owner, zero),
            EventType::Read | EventType::Ensure => (zero, zero),
            EventType::GiveOwner => (event_owner, event_value),
            EventType::TakeOwner => (event_value, event_owner),
        };

        let (old_data, new_data) = match event_ty {
            EventType::Write | EventType::Ensure => (zero, event_value),
            EventType::Read => (event_value, zero),
            EventType::GiveOwner | EventType::TakeOwner | EventType::CreditDelta => (zero, zero),
        };

        Self {
            address,
            object_flags,
            old_owner,
            new_owner,
            old_data,
            new_data,
            credit_delta,
            event_owner,
            event_ty: F::from_canonical_u64(event_ty as u64),
            event_value,
        }
    }
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(
        &self,
        witness: &mut PartialWitness<F>,
        address: u64,
        event_owner: [F; 4],
        event_ty: EventType,
        event_value: [F; 4],
    ) {
        self.set_inputs_unsafe(
            witness,
            LeafWitnessValue::from_event(address, event_owner, event_ty, event_value),
        );
    }

    pub fn set_inputs_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        v: LeafWitnessValue<F>,
    ) {
        inputs.set_target(
            self.targets.inputs.address,
            F::from_canonical_u64(v.address),
        );
        inputs.set_target(
            self.targets.inputs.object_flags,
            F::from_canonical_u8(v.object_flags.bits()),
        );
        inputs.set_target_arr(&self.targets.inputs.old_owner, &v.old_owner);
        inputs.set_target_arr(&self.targets.inputs.new_owner, &v.new_owner);
        inputs.set_target_arr(&self.targets.inputs.old_data, &v.old_data);
        inputs.set_target_arr(&self.targets.inputs.new_data, &v.new_data);
        inputs.set_target(
            self.targets.inputs.credit_delta,
            F::from_noncanonical_i64(v.credit_delta),
        );
        inputs.set_target_arr(&self.targets.event_owner, &v.event_owner);
        inputs.set_target(self.targets.event_ty, v.event_ty);
        inputs.set_target_arr(&self.targets.event_value, &v.event_value);
    }
}

pub struct BranchTargets {
    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,

    /// This public inputs
    pub parent: SubCircuitInputs,
}

impl SubCircuitInputs {
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let address = indices.get_address(&proof.public_inputs);
        let object_flags = indices.get_object_flags(&proof.public_inputs);
        let old_owner = indices.get_old_owner(&proof.public_inputs);
        let new_owner = indices.get_new_owner(&proof.public_inputs);
        let old_data = indices.get_old_data(&proof.public_inputs);
        let new_data = indices.get_new_data(&proof.public_inputs);
        let credit_delta = indices.get_credit_delta(&proof.public_inputs);

        SubCircuitInputs {
            address,
            object_flags,
            old_owner,
            new_owner,
            old_data,
            new_data,
            credit_delta,
        }
    }

    fn build_helper<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        left: SubCircuitInputs,
        right: SubCircuitInputs,
    ) -> BranchTargets {
        builder.connect(self.address, left.address);
        builder.connect(self.address, right.address);

        // Split up the flags
        let parent_flags = SplitFlags::split(builder, self.object_flags);
        let left_flags = SplitFlags::split(builder, left.object_flags);
        let right_flags = SplitFlags::split(builder, right.object_flags);

        // These flags can only be set once, so we can just add
        let write_flag_calc = builder.add(left_flags.write.target, right_flags.write.target);
        let give_owner_flag_calc =
            builder.add(left_flags.give_owner.target, right_flags.give_owner.target);
        let take_owner_flag_calc =
            builder.add(left_flags.take_owner.target, right_flags.take_owner.target);
        builder.connect(write_flag_calc, parent_flags.write.target);
        builder.connect(give_owner_flag_calc, parent_flags.give_owner.target);
        builder.connect(take_owner_flag_calc, parent_flags.take_owner.target);

        // These flags can be set multiple times, so we must use `or`
        let ensure_flag_calc = builder.or(left_flags.ensure, right_flags.ensure);
        let read_flag_calc = builder.or(left_flags.read, right_flags.read);
        builder.connect(ensure_flag_calc.target, parent_flags.ensure.target);
        builder.connect(read_flag_calc.target, parent_flags.read.target);

        // Presence check for matching object fields
        let left_has_new_data = builder.is_nonzero(left_flags.new_data);
        let right_has_new_data = builder.is_nonzero(right_flags.new_data);
        let left_has_owner = builder.is_nonzero(left_flags.owner);
        let right_has_owner = builder.is_nonzero(right_flags.owner);
        let left_has_old_owner = builder.or(left_flags.write, left_has_owner);
        let right_has_old_owner = builder.or(right_flags.write, right_has_owner);

        // Enforce restrictions on all the object fields
        maybe_connect(builder, self.old_owner, left_has_old_owner, left.old_owner);
        maybe_connect(
            builder,
            self.old_owner,
            right_has_old_owner,
            right.old_owner,
        );

        maybe_connect(builder, self.new_owner, left_has_owner, left.new_owner);
        maybe_connect(builder, self.new_owner, right_has_owner, right.new_owner);

        maybe_connect(builder, self.old_data, left_flags.read, left.old_data);
        maybe_connect(builder, self.old_data, right_flags.read, right.old_data);

        maybe_connect(builder, self.new_data, left_has_new_data, left.new_data);
        maybe_connect(builder, self.new_data, right_has_new_data, right.new_data);

        let credit_delta_calc = builder.add(left.credit_delta, right.credit_delta);
        builder.connect(credit_delta_calc, self.credit_delta);

        BranchTargets {
            left,
            right,
            parent: self,
        }
    }

    #[must_use]
    pub fn from_leaf<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        leaf: &LeafSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, &leaf.indices);
        let right = Self::direction_from_node(right_proof, &leaf.indices);
        self.build_helper(builder, left, right)
    }

    pub fn from_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        branch: &BranchSubCircuit,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let left = Self::direction_from_node(left_proof, &branch.indices);
        let right = Self::direction_from_node(right_proof, &branch.indices);
        self.build_helper(builder, left, right)
    }
}

/// The branch subcircuit metadata. This subcircuit validates the merge of two
/// (private) partial object does not conflict.
pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
    /// The distance from the leaves (`0` being the lowest branch)
    /// Used for debugging
    pub dbg_height: usize,
}

impl BranchTargets {
    fn get_indices(&self, public_inputs: &[Target]) -> PublicIndices {
        PublicIndices {
            address: public_inputs
                .iter()
                .position(|&pi| pi == self.parent.address)
                .expect("target not found"),
            object_flags: public_inputs
                .iter()
                .position(|&pi| pi == self.parent.object_flags)
                .expect("target not found"),
            old_owner: self.parent.old_owner.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            new_owner: self.parent.new_owner.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            old_data: self.parent.old_data.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            new_data: self.parent.new_data.map(|target| {
                public_inputs
                    .iter()
                    .position(|&pi| pi == target)
                    .expect("target not found")
            }),
            credit_delta: public_inputs
                .iter()
                .position(|&pi| pi == self.parent.credit_delta)
                .expect("target not found"),
        }
    }

    #[must_use]
    pub fn from_leaf(self, public_inputs: &[Target]) -> BranchSubCircuit {
        BranchSubCircuit {
            indices: self.get_indices(public_inputs),
            targets: self,
            dbg_height: 0,
        }
    }

    #[must_use]
    pub fn from_branch(
        self,
        branch: &BranchSubCircuit,
        public_inputs: &[Target],
    ) -> BranchSubCircuit {
        BranchSubCircuit {
            indices: self.get_indices(public_inputs),
            targets: self,
            dbg_height: branch.dbg_height + 1,
        }
    }
}

impl<F> From<LeafWitnessValue<F>> for BranchWitnessValue<F> {
    fn from(value: LeafWitnessValue<F>) -> Self {
        Self {
            address: value.address,
            object_flags: value.object_flags,
            old_owner: value.old_owner,
            new_owner: value.new_owner,
            old_data: value.old_data,
            new_data: value.new_data,
            credit_delta: value.credit_delta,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BranchWitnessValue<F> {
    pub address: u64,
    pub object_flags: BitFlags<Flags>,
    pub old_owner: [F; 4],
    pub new_owner: [F; 4],
    pub old_data: [F; 4],
    pub new_data: [F; 4],
    pub credit_delta: i64,
}

fn branch_helper<F: RichField>(
    l: &BranchWitnessValue<F>,
    r: &BranchWitnessValue<F>,
    f: impl Fn(&BranchWitnessValue<F>) -> [F; 4],
    flags: impl Into<BitFlags<Flags>>,
) -> [F; 4] {
    let flags = flags.into();
    match (
        l.object_flags.intersects(flags),
        r.object_flags.intersects(flags),
    ) {
        (false, false) => [F::ZERO; 4],
        (true, false) => f(l),
        (false, true) => f(r),
        (true, true) => {
            let l = f(l);
            debug_assert_eq!(l, f(r));
            l
        }
    }
}

impl<F: RichField> BranchWitnessValue<F> {
    pub fn from_branches(left: impl Into<Self>, right: impl Into<Self>) -> Self {
        let left: Self = left.into();
        let right: Self = right.into();

        let old_owner = Flags::WriteFlag | Flags::GiveOwnerFlag | Flags::TakeOwnerFlag;
        let new_owner = Flags::GiveOwnerFlag | Flags::TakeOwnerFlag;
        let new_data = Flags::ReadFlag;
        let old_data = Flags::WriteFlag | Flags::EnsureFlag;

        Self {
            address: left.address,
            object_flags: left.object_flags | right.object_flags,
            old_owner: branch_helper(&left, &right, |c| c.old_owner, old_owner),
            new_owner: branch_helper(&left, &right, |c| c.new_owner, new_owner),
            old_data: branch_helper(&left, &right, |c| c.old_data, old_data),
            new_data: branch_helper(&left, &right, |c| c.new_data, new_data),
            credit_delta: left.credit_delta + right.credit_delta,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(
        &self,
        witness: &mut PartialWitness<F>,
        v: BranchWitnessValue<F>,
    ) {
        witness.set_target(
            self.targets.parent.address,
            F::from_canonical_u64(v.address),
        );
        witness.set_target(
            self.targets.parent.object_flags,
            F::from_canonical_u8(v.object_flags.bits()),
        );
        witness.set_target_arr(&self.targets.parent.old_owner, &v.old_owner);
        witness.set_target_arr(&self.targets.parent.new_owner, &v.new_owner);
        witness.set_target_arr(&self.targets.parent.old_data, &v.old_data);
        witness.set_target_arr(&self.targets.parent.new_data, &v.new_data);
        witness.set_target(
            self.targets.parent.credit_delta,
            F::from_noncanonical_i64(v.credit_delta),
        );
    }
}

#[cfg(test)]
mod test {
    use std::panic::{catch_unwind, UnwindSafe};

    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::recproof::unbounded;
    use crate::test_utils::{C, D, F};

    pub struct DummyLeafCircuit {
        pub state_from_events: LeafSubCircuit,
        pub unbounded: unbounded::LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let state_from_events_inputs = SubCircuitInputs::default(&mut builder);
            let state_from_events_targets = state_from_events_inputs.build_leaf(&mut builder);
            let (circuit, unbounded) = unbounded::LeafSubCircuit::new(builder);
            let state_from_events =
                state_from_events_targets.build(&circuit.prover_only.public_inputs);

            Self {
                state_from_events,
                unbounded,
                circuit,
            }
        }

        pub fn prove(
            &self,
            branch: &DummyBranchCircuit,
            address: u64,
            event_owner: [F; 4],
            event_ty: EventType,
            event_value: [F; 4],
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.state_from_events.set_witness(
                &mut inputs,
                address,
                event_owner,
                event_ty,
                event_value,
            );
            self.unbounded.set_inputs(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }

        #[allow(dead_code)]
        fn prove_unsafe(
            &self,
            branch: &DummyBranchCircuit,
            v: LeafWitnessValue<F>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.state_from_events.set_inputs_unsafe(&mut inputs, v);
            self.unbounded.set_inputs(&mut inputs, &branch.circuit);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub state_from_events: BranchSubCircuit,
        pub unbounded: unbounded::BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
        pub targets: DummyBranchTargets,
    }

    pub struct DummyBranchTargets {
        pub left_is_leaf: BoolTarget,
        pub right_is_leaf: BoolTarget,
        pub left_proof: ProofWithPublicInputsTarget<D>,
        pub right_proof: ProofWithPublicInputsTarget<D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());
            let common = &leaf.circuit.common;
            let left_proof = builder.add_virtual_proof_with_pis(common);
            let right_proof = builder.add_virtual_proof_with_pis(common);
            let left_is_leaf = builder.add_virtual_bool_target_safe();
            let right_is_leaf = builder.add_virtual_bool_target_safe();

            let state_from_events_inputs = SubCircuitInputs::default(&mut builder);

            let state_from_events_targets = state_from_events_inputs.from_leaf(
                &mut builder,
                &leaf.state_from_events,
                &left_proof,
                &right_proof,
            );
            let (circuit, unbounded) = unbounded::BranchSubCircuit::new(
                builder,
                &leaf.circuit,
                left_is_leaf,
                right_is_leaf,
                &left_proof,
                &right_proof,
            );

            let targets = DummyBranchTargets {
                left_is_leaf,
                right_is_leaf,
                left_proof,
                right_proof,
            };
            let state_from_events =
                state_from_events_targets.from_leaf(&circuit.prover_only.public_inputs);

            Self {
                state_from_events,
                unbounded,
                circuit,
                targets,
            }
        }

        pub fn prove(
            &self,
            v: BranchWitnessValue<F>,
            left_is_leaf: bool,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_is_leaf: bool,
            right_proof: &ProofWithPublicInputs<F, C, D>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.state_from_events.set_witness(&mut inputs, v);
            inputs.set_bool_target(self.targets.left_is_leaf, left_is_leaf);
            inputs.set_bool_target(self.targets.right_is_leaf, right_is_leaf);
            inputs.set_proof_with_pis_target(&self.targets.left_proof, left_proof);
            inputs.set_proof_with_pis_target(&self.targets.right_proof, right_proof);
            self.circuit.prove(inputs)
        }
    }

    #[test]
    fn verify_leaf() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);
        let program_hash_2 = [2, 3, 4, 2].map(F::from_canonical_u64);

        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [42, 0, 0, 0].map(F::from_canonical_u64);
        let non_zero_val_3 = [42, 0, 0, 1].map(F::from_canonical_u64);

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::Write,
            non_zero_val_1,
        )?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::Read,
            non_zero_val_1,
        )?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::Ensure,
            non_zero_val_1,
        )?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::GiveOwner,
            program_hash_2,
        )?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_2,
            EventType::TakeOwner,
            program_hash_1,
        )?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::CreditDelta,
            non_zero_val_2,
        )?;
        leaf.circuit.verify(proof)?;

        let proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::CreditDelta,
            non_zero_val_3,
        )?;
        leaf.circuit.verify(proof)?;

        Ok(())
    }

    fn leaf_test_helper<Fn>(owner: [u64; 4], event_ty: EventType, value: [u64; 4], f: Fn)
    where
        Fn: FnOnce(&mut LeafWitnessValue<F>, [F; 4], [F; 4]) + UnwindSafe, {
        let (leaf, branch, event) = catch_unwind(|| {
            let circuit_config = CircuitConfig::standard_recursion_config();
            let leaf = DummyLeafCircuit::new(&circuit_config);
            let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

            let owner = owner.map(F::from_canonical_u64);
            let value = value.map(F::from_canonical_u64);

            let mut event = LeafWitnessValue::from_event(200, owner, event_ty, value);

            f(&mut event, owner, value);

            (leaf, branch, event)
        })
        .expect("shouldn't fail");

        leaf.prove_unsafe(&branch, event).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_1() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = Flags::EnsureFlag.into();
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_2() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = Flags::GiveOwnerFlag.into();
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_3() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = Flags::EnsureFlag | Flags::WriteFlag;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_4() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.credit_delta = 5;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_ensure_leaf_1() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::Ensure,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = Flags::WriteFlag.into();
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_give_leaf_1() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::GiveOwner,
            [3, 1, 4, 15],
            |event, owner, _| {
                event.new_owner = owner;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_give_leaf_2() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::GiveOwner,
            [3, 1, 4, 15],
            |event, _, value| {
                event.old_owner = value;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_credit_leaf_1() {
        leaf_test_helper(
            [4, 8, 15, 16],
            EventType::CreditDelta,
            [13, 0, 0, 0],
            |event, _, _| {
                event.credit_delta *= -1;
            },
        );
    }

    struct NoFn;

    trait MaybeBfn: Sized + UnwindSafe {
        const PRESENT: bool;
        fn apply(self, _event: &mut BranchWitnessValue<F>) { unimplemented!() }
    }

    impl MaybeBfn for NoFn {
        const PRESENT: bool = false;
    }
    impl<Fn: FnOnce(&mut BranchWitnessValue<F>) + UnwindSafe> MaybeBfn for Fn {
        const PRESENT: bool = true;

        fn apply(self, event: &mut BranchWitnessValue<F>) { self(event) }
    }

    fn branch_test_helper<Lfn, Bfn1, Bfn2>(
        owners: [[u64; 4]; 3],
        tys: [EventType; 3],
        values: [[u64; 4]; 3],
        lf: Lfn,
        bf1: Bfn1,
        bf2: Bfn2,
    ) where
        Lfn: FnOnce(&mut LeafWitnessValue<F>, &mut LeafWitnessValue<F>, &mut LeafWitnessValue<F>)
            + UnwindSafe,
        Bfn1: FnOnce(&mut BranchWitnessValue<F>) + UnwindSafe,
        Bfn2: MaybeBfn, {
        let (branch, left, right, branch_event) = catch_unwind(|| {
            let circuit_config = CircuitConfig::standard_recursion_config();
            let leaf = DummyLeafCircuit::new(&circuit_config);
            let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

            let owners = owners.map(|owner| owner.map(F::from_canonical_u64));
            let values = values.map(|owner| owner.map(F::from_canonical_u64));

            let mut event0 = LeafWitnessValue::from_event(200, owners[0], tys[0], values[0]);
            let mut event1 = LeafWitnessValue::from_event(200, owners[1], tys[1], values[1]);
            let mut event2 = LeafWitnessValue::from_event(200, owners[2], tys[2], values[2]);
            lf(&mut event0, &mut event1, &mut event2);

            let mut branch_event_1 = BranchWitnessValue::from_branches(event0, event1);
            bf1(&mut branch_event_1);
            let mut branch_event_2 = BranchWitnessValue::from_branches(branch_event_1, event2);
            if Bfn2::PRESENT {
                bf2.apply(&mut branch_event_2);
            };

            let leaf_proof_array =
                [event0, event1, event2].map(|event| leaf.prove_unsafe(&branch, event).unwrap());
            let _ = leaf_proof_array
                .clone()
                .map(|proof| leaf.circuit.verify(proof).unwrap());

            let [leaf_proof0, leaf_proof1, leaf_proof2] = leaf_proof_array;

            let (left, right, branch_event) = if Bfn2::PRESENT {
                let branch_proof_1 = branch
                    .prove(branch_event_1, true, &leaf_proof0, true, &leaf_proof1)
                    .unwrap();
                branch.circuit.verify(branch_proof_1.clone()).unwrap();

                ((false, branch_proof_1), (true, leaf_proof2), branch_event_2)
            } else {
                ((true, leaf_proof0), (true, leaf_proof1), branch_event_1)
            };
            (branch, left, right, branch_event)
        })
        .expect("shouldn't fail");

        branch
            .prove(branch_event, left.0, &left.1, right.0, &right.1)
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_mismatch_address_1() {
        branch_test_helper(
            [[4, 8, 15, 16], [2, 3, 4, 2], [2, 3, 4, 2]],
            [EventType::Write, EventType::Read, EventType::Ensure],
            [[3, 1, 4, 15], [1, 6, 180, 33], [3, 1, 4, 15]],
            |_, _, event| {
                event.address += 10;
            },
            |_| {},
            |_: &mut _| {},
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_mismatch_address_2() {
        branch_test_helper(
            [[4, 8, 15, 16], [2, 3, 4, 2], [2, 3, 4, 2]],
            [EventType::Write, EventType::Read, EventType::Ensure],
            [[3, 1, 4, 15], [1, 6, 180, 33], [3, 1, 4, 15]],
            |_, _, _| {},
            |_| {},
            |event: &mut BranchWitnessValue<F>| {
                event.address += 10;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_double_write() {
        branch_test_helper(
            [[4, 8, 15, 16], [4, 8, 15, 16], [2, 3, 4, 2]],
            [EventType::Write, EventType::Write, EventType::Ensure],
            [[3, 1, 4, 15], [1, 6, 180, 33], [3, 1, 4, 15]],
            |_, _, _| {},
            |_| {},
            NoFn,
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_double_credit_sum() {
        branch_test_helper(
            [[4, 8, 15, 16], [4, 8, 15, 16], [2, 3, 4, 2]],
            [
                EventType::CreditDelta,
                EventType::CreditDelta,
                EventType::Ensure,
            ],
            [[13, 0, 0, 0], [8, 0, 0, 1], [3, 1, 4, 15]],
            |_, _, _| {},
            |_| {},
            |event: &mut BranchWitnessValue<F>| {
                event.credit_delta += 10;
            },
        );
    }

    #[test]
    fn verify_branch() -> Result<()> {
        let circuit_config = CircuitConfig::standard_recursion_config();
        let leaf = DummyLeafCircuit::new(&circuit_config);
        let branch = DummyBranchCircuit::new(&circuit_config, &leaf);

        let program_hash_1 = [4, 8, 15, 16].map(F::from_canonical_u64);

        let zero_val = [F::ZERO; 4];
        let non_zero_val_1 = [3, 1, 4, 15].map(F::from_canonical_u64);
        let non_zero_val_2 = [1, 6, 180, 33].map(F::from_canonical_u64);

        let read_proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::Read,
            non_zero_val_1,
        )?;
        leaf.circuit.verify(read_proof.clone())?;

        let write_proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::Write,
            non_zero_val_2,
        )?;
        leaf.circuit.verify(write_proof.clone())?;

        let ensure_proof = leaf.prove(
            &branch,
            200,
            program_hash_1,
            EventType::Ensure,
            non_zero_val_2,
        )?;
        leaf.circuit.verify(ensure_proof.clone())?;

        let branch_proof_1 = branch.prove(
            BranchWitnessValue {
                address: 200,
                object_flags: Flags::ReadFlag | Flags::WriteFlag,
                old_owner: program_hash_1,
                new_owner: zero_val,
                old_data: non_zero_val_1,
                new_data: non_zero_val_2,
                credit_delta: 0,
            },
            true,
            &read_proof,
            true,
            &write_proof,
        )?;
        branch.circuit.verify(branch_proof_1.clone())?;

        let branch_proof_2 = branch.prove(
            BranchWitnessValue {
                address: 200,
                object_flags: Flags::ReadFlag | Flags::WriteFlag | Flags::EnsureFlag,
                old_owner: program_hash_1,
                new_owner: zero_val,
                old_data: non_zero_val_1,
                new_data: non_zero_val_2,
                credit_delta: 0,
            },
            false,
            &branch_proof_1,
            true,
            &ensure_proof,
        )?;
        branch.circuit.verify(branch_proof_2)?;

        Ok(())
    }
}
