//! Subcircuits for proving events can be accumulated to a state delta object.

use enumflags2::BitFlags;
use itertools::{chain, Itertools};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::proof::ProofWithPublicInputsTarget;

use crate::indices::{ArrayTargetIndex, TargetIndex};
use crate::{maybe_connect, Event, EventFlags, EventType};

// Limit transfers to 2^40 credits to avoid overflow issues
const MAX_LEAF_TRANSFER: usize = 40;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PublicIndices {
    /// The index of the event/object address
    pub address: TargetIndex,

    /// The index of the (partial) object flags
    pub object_flags: TargetIndex,

    /// The indices of each of the elements of the previous constraint owner
    pub old_owner: ArrayTargetIndex<TargetIndex, 4>,

    /// The indices of each of the elements of the new constraint owner
    pub new_owner: ArrayTargetIndex<TargetIndex, 4>,

    /// The indices of each of the elements of the previous data
    pub old_data: ArrayTargetIndex<TargetIndex, 4>,

    /// The indices of each of the elements of the new data
    pub new_data: ArrayTargetIndex<TargetIndex, 4>,

    /// The index of the credit delta
    pub credit_delta: TargetIndex,
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

pub struct SplitFlags {
    pub new_data: Target,
    pub owner: Target,

    pub write: BoolTarget,
    pub ensure: BoolTarget,
    pub read: BoolTarget,
    pub give_owner: BoolTarget,
    pub take_owner: BoolTarget,
}

impl SplitFlags {
    pub fn split<F, const D: usize>(builder: &mut CircuitBuilder<F, D>, flags: Target) -> Self
    where
        F: RichField + Extendable<D>, {
        let new_data_flag_count = (EventFlags::WriteFlag | EventFlags::EnsureFlag).len();
        let other_flag_count = EventFlags::count() - new_data_flag_count;
        let owner_flag_count = (EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag).len();

        // Split off the flags corresponding to changes to new_data
        let (new_data, flags) =
            builder.split_low_high(flags, new_data_flag_count, EventFlags::count());
        // Split  the flag corresponding to read from the owner flags
        let (read, owner) = builder.split_low_high(flags, 1, other_flag_count);
        let read = BoolTarget::new_unsafe(read);

        let new_data_flags = builder.split_le(new_data, new_data_flag_count);
        let owner_flags = builder.split_le(owner, owner_flag_count);

        let flags = chain!(new_data_flags, [read], owner_flags).collect_vec();

        Self {
            new_data,
            owner,
            write: flags[EventFlags::WriteFlag.index()],
            ensure: flags[EventFlags::EnsureFlag.index()],
            read: flags[EventFlags::ReadFlag.index()],
            give_owner: flags[EventFlags::GiveOwnerFlag.index()],
            take_owner: flags[EventFlags::TakeOwnerFlag.index()],
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
            let object_flags = builder.split_le(self.object_flags, EventFlags::count());

            (
                object_flags[EventFlags::WriteFlag.index()],
                object_flags[EventFlags::EnsureFlag.index()],
                object_flags[EventFlags::ReadFlag.index()],
                object_flags[EventFlags::GiveOwnerFlag.index()],
                object_flags[EventFlags::TakeOwnerFlag.index()],
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
        // The sign can only be zero or one
        builder.assert_bool(BoolTarget::new_unsafe(credit_delta_sign));
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
        // Find the indices
        let indices = PublicIndices {
            address: TargetIndex::new(public_inputs, self.inputs.address),
            object_flags: TargetIndex::new(public_inputs, self.inputs.object_flags),
            old_owner: ArrayTargetIndex::new(public_inputs, &self.inputs.old_owner),
            new_owner: ArrayTargetIndex::new(public_inputs, &self.inputs.new_owner),
            old_data: ArrayTargetIndex::new(public_inputs, &self.inputs.old_data),
            new_data: ArrayTargetIndex::new(public_inputs, &self.inputs.new_data),
            credit_delta: TargetIndex::new(public_inputs, self.inputs.credit_delta),
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
    pub object_flags: BitFlags<EventFlags>,
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
    pub fn from_event(event: Event<F>) -> Self {
        let zero = [F::ZERO; 4];

        let (object_flags, credit_delta) = match event.ty {
            EventType::Write => (EventFlags::WriteFlag.into(), 0),
            EventType::Read => (EventFlags::ReadFlag.into(), 0),
            EventType::Ensure => (EventFlags::EnsureFlag.into(), 0),
            EventType::GiveOwner => (EventFlags::GiveOwnerFlag.into(), 0),
            EventType::TakeOwner => (EventFlags::TakeOwnerFlag.into(), 0),
            EventType::CreditDelta => {
                #[allow(clippy::cast_possible_wrap)]
                let value = event.value[0].to_canonical_u64() as i64;
                let value = match event.value[3] {
                    sign if sign.is_zero() => value,
                    sign if sign.is_one() => -value,
                    _ => unreachable!(),
                };
                (BitFlags::EMPTY, value)
            }
        };

        let (old_owner, new_owner) = match event.ty {
            EventType::Write | EventType::CreditDelta => (event.owner, zero),
            EventType::Read | EventType::Ensure => (zero, zero),
            EventType::GiveOwner => (event.owner, event.value),
            EventType::TakeOwner => (event.value, event.owner),
        };

        let (old_data, new_data) = match event.ty {
            EventType::Write | EventType::Ensure => (zero, event.value),
            EventType::Read => (event.value, zero),
            EventType::GiveOwner | EventType::TakeOwner | EventType::CreditDelta => (zero, zero),
        };

        Self {
            address: event.address,
            object_flags,
            old_owner,
            new_owner,
            old_data,
            new_data,
            credit_delta,
            event_owner: event.owner,
            event_ty: F::from_canonical_u64(event.ty as u64),
            event_value: event.value,
        }
    }
}

impl LeafSubCircuit {
    /// Get ready to generate a proof
    pub fn set_witness<F: RichField>(&self, witness: &mut PartialWitness<F>, event: Event<F>) {
        self.set_witness_unsafe(witness, LeafWitnessValue::from_event(event));
    }

    pub fn set_witness_unsafe<F: RichField>(
        &self,
        inputs: &mut PartialWitness<F>,
        v: LeafWitnessValue<F>,
    ) {
        let targets = &self.targets.inputs;
        inputs.set_target(targets.address, F::from_canonical_u64(v.address));
        inputs.set_target(
            targets.object_flags,
            F::from_canonical_u8(v.object_flags.bits()),
        );
        inputs.set_target_arr(&targets.old_owner, &v.old_owner);
        inputs.set_target_arr(&targets.new_owner, &v.new_owner);
        inputs.set_target_arr(&targets.old_data, &v.old_data);
        inputs.set_target_arr(&targets.new_data, &v.new_data);
        inputs.set_target(
            targets.credit_delta,
            F::from_noncanonical_i64(v.credit_delta),
        );
        inputs.set_target_arr(&self.targets.event_owner, &v.event_owner);
        inputs.set_target(self.targets.event_ty, v.event_ty);
        inputs.set_target_arr(&self.targets.event_value, &v.event_value);
    }
}

pub struct BranchTargets {
    /// This public inputs
    pub inputs: SubCircuitInputs,

    /// The left direction
    pub left: SubCircuitInputs,

    /// The right direction
    pub right: SubCircuitInputs,

    /// Whether or not the right direction is present
    pub partial: BoolTarget,
}

impl SubCircuitInputs {
    fn direction_from_node<const D: usize>(
        proof: &ProofWithPublicInputsTarget<D>,
        indices: &PublicIndices,
    ) -> SubCircuitInputs {
        let address = indices.address.get_target(&proof.public_inputs);
        let object_flags = indices.object_flags.get_target(&proof.public_inputs);
        let old_owner = indices.old_owner.get_target(&proof.public_inputs);
        let new_owner = indices.new_owner.get_target(&proof.public_inputs);
        let old_data = indices.old_data.get_target(&proof.public_inputs);
        let new_data = indices.new_data.get_target(&proof.public_inputs);
        let credit_delta = indices.credit_delta.get_target(&proof.public_inputs);

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

    #[must_use]
    pub fn build_branch<F: RichField + Extendable<D>, const D: usize>(
        self,
        builder: &mut CircuitBuilder<F, D>,
        indices: &PublicIndices,
        left_proof: &ProofWithPublicInputsTarget<D>,
        right_proof: &ProofWithPublicInputsTarget<D>,
    ) -> BranchTargets {
        let zero = builder.zero();

        let left = Self::direction_from_node(left_proof, indices);
        let mut right = Self::direction_from_node(right_proof, indices);

        // Possibly clear the right side (partial)
        let partial = builder.add_virtual_bool_target_safe();
        right.object_flags = builder.select(partial, zero, right.object_flags);
        right.credit_delta = builder.select(partial, zero, right.credit_delta);

        // Match addresses
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
            inputs: self,
            left,
            right,
            partial,
        }
    }
}

/// The branch subcircuit metadata. This subcircuit validates the merge of two
/// (private) partial object does not conflict.
pub struct BranchSubCircuit {
    pub targets: BranchTargets,
    pub indices: PublicIndices,
}

impl BranchTargets {
    #[must_use]
    pub fn build(self, child: &PublicIndices, public_inputs: &[Target]) -> BranchSubCircuit {
        // Find the indices
        let indices = PublicIndices {
            address: TargetIndex::new(public_inputs, self.inputs.address),
            object_flags: TargetIndex::new(public_inputs, self.inputs.object_flags),
            old_owner: ArrayTargetIndex::new(public_inputs, &self.inputs.old_owner),
            new_owner: ArrayTargetIndex::new(public_inputs, &self.inputs.new_owner),
            old_data: ArrayTargetIndex::new(public_inputs, &self.inputs.old_data),
            new_data: ArrayTargetIndex::new(public_inputs, &self.inputs.new_data),
            credit_delta: TargetIndex::new(public_inputs, self.inputs.credit_delta),
        };
        debug_assert_eq!(indices, *child);

        BranchSubCircuit {
            indices,
            targets: self,
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
    pub object_flags: BitFlags<EventFlags>,
    pub old_owner: [F; 4],
    pub new_owner: [F; 4],
    pub old_data: [F; 4],
    pub new_data: [F; 4],
    pub credit_delta: i64,
}

fn merge_branch<F: RichField>(
    l: &BranchWitnessValue<F>,
    r: &BranchWitnessValue<F>,
    f: impl Fn(&BranchWitnessValue<F>) -> [F; 4],
    flags: impl Into<BitFlags<EventFlags>>,
) -> [F; 4] {
    merge_branch_helper(l, l.object_flags, r, r.object_flags, f, flags)
}

fn merge_branch_helper<F: RichField, W>(
    l: W,
    l_flags: BitFlags<EventFlags>,
    r: W,
    r_flags: BitFlags<EventFlags>,
    f: impl Fn(W) -> [F; 4],
    flags: impl Into<BitFlags<EventFlags>>,
) -> [F; 4] {
    let flags = flags.into();
    match (l_flags.intersects(flags), r_flags.intersects(flags)) {
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

        let old_owner =
            EventFlags::WriteFlag | EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag;
        let new_owner = EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag;
        let old_data = EventFlags::ReadFlag;
        let new_data = EventFlags::WriteFlag | EventFlags::EnsureFlag;

        Self {
            address: left.address,
            object_flags: left.object_flags | right.object_flags,
            old_owner: merge_branch(&left, &right, |c| c.old_owner, old_owner),
            new_owner: merge_branch(&left, &right, |c| c.new_owner, new_owner),
            old_data: merge_branch(&left, &right, |c| c.old_data, old_data),
            new_data: merge_branch(&left, &right, |c| c.new_data, new_data),
            credit_delta: left.credit_delta + right.credit_delta,
        }
    }
}

impl BranchSubCircuit {
    pub fn set_witness<F: RichField>(
        &self,
        witness: &mut PartialWitness<F>,
        partial: bool,
        v: BranchWitnessValue<F>,
    ) {
        let targets = &self.targets.inputs;
        witness.set_target(targets.address, F::from_canonical_u64(v.address));
        witness.set_target(
            targets.object_flags,
            F::from_canonical_u8(v.object_flags.bits()),
        );
        witness.set_target_arr(&targets.old_owner, &v.old_owner);
        witness.set_target_arr(&targets.new_owner, &v.new_owner);
        witness.set_target_arr(&targets.old_data, &v.old_data);
        witness.set_target_arr(&targets.new_data, &v.new_data);
        witness.set_target(
            targets.credit_delta,
            F::from_noncanonical_i64(v.credit_delta),
        );
        witness.set_bool_target(self.targets.partial, partial);
    }

    pub fn set_witness_from_proof<F: RichField>(
        &self,
        witness: &mut PartialWitness<F>,
        left_inputs: &[F],
    ) {
        let targets = &self.targets.inputs;
        let indices = &self.indices;
        witness.set_target(
            targets.object_flags,
            indices.object_flags.get_field(left_inputs),
        );
        witness.set_target_arr(
            &targets.old_owner,
            &indices.old_owner.get_field(left_inputs),
        );
        witness.set_target_arr(
            &targets.new_owner,
            &indices.new_owner.get_field(left_inputs),
        );
        witness.set_target_arr(&targets.old_data, &indices.old_data.get_field(left_inputs));
        witness.set_target_arr(&targets.new_data, &indices.new_data.get_field(left_inputs));
        witness.set_target(
            targets.credit_delta,
            indices.credit_delta.get_field(left_inputs),
        );
        witness.set_bool_target(self.targets.partial, true);
    }

    pub fn set_witness_from_proofs<F: RichField>(
        &self,
        witness: &mut PartialWitness<F>,
        left_inputs: &[F],
        right_inputs: &[F],
    ) {
        let targets = &self.targets.inputs;
        let indices = &self.indices;

        // Address can be derived, so we can skip it

        // Handle flags
        let left_flags = indices
            .object_flags
            .get_field(left_inputs)
            .to_canonical_u64();
        let right_flags = indices
            .object_flags
            .get_field(right_inputs)
            .to_canonical_u64();
        witness.set_target(
            targets.object_flags,
            F::from_canonical_u64(left_flags | right_flags),
        );
        #[allow(clippy::cast_possible_truncation)]
        let left_flags = BitFlags::<EventFlags>::from_bits(left_flags as u8).unwrap();
        #[allow(clippy::cast_possible_truncation)]
        let right_flags = BitFlags::<EventFlags>::from_bits(right_flags as u8).unwrap();

        // Setup the flags for each scenario
        let old_owner =
            EventFlags::WriteFlag | EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag;
        let new_owner = EventFlags::GiveOwnerFlag | EventFlags::TakeOwnerFlag;
        let old_data = EventFlags::ReadFlag;
        let new_data = EventFlags::WriteFlag | EventFlags::EnsureFlag;

        // Get the object fields based on the flags
        let old_owner = merge_branch_helper(
            left_inputs,
            left_flags,
            right_inputs,
            right_flags,
            |inputs| indices.old_owner.get_field(inputs),
            old_owner,
        );
        let new_owner = merge_branch_helper(
            left_inputs,
            left_flags,
            right_inputs,
            right_flags,
            |inputs| indices.new_owner.get_field(inputs),
            new_owner,
        );
        let old_data = merge_branch_helper(
            left_inputs,
            left_flags,
            right_inputs,
            right_flags,
            |inputs| indices.old_data.get_field(inputs),
            old_data,
        );
        let new_data = merge_branch_helper(
            left_inputs,
            left_flags,
            right_inputs,
            right_flags,
            |inputs| indices.new_data.get_field(inputs),
            new_data,
        );

        // Set the object fields
        witness.set_target_arr(&targets.old_owner, &old_owner);
        witness.set_target_arr(&targets.new_owner, &new_owner);
        witness.set_target_arr(&targets.old_data, &old_data);
        witness.set_target_arr(&targets.new_data, &new_data);

        // Handle the credits
        let left_credits = indices.credit_delta.get_field(left_inputs);
        let right_credits = indices.credit_delta.get_field(right_inputs);
        let credits = left_credits + right_credits;
        witness.set_target(targets.credit_delta, credits);

        // Both sides, so not partial
        witness.set_bool_target(self.targets.partial, false);
    }
}

#[cfg(test)]
mod test {
    use std::cell::Cell;
    use std::panic::{catch_unwind, UnwindSafe};

    use anyhow::Result;
    use plonky2::field::types::Field;
    use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData};
    use plonky2::plonk::proof::ProofWithPublicInputs;

    use super::*;
    use crate::circuits::test_data::{
        ADDRESS_A, EVENT_T0_P0_A_CREDIT, EVENT_T0_P0_A_WRITE, EVENT_T0_P2_A_ENSURE,
        EVENT_T0_P2_A_READ, EVENT_T0_P2_C_TAKE, EVENT_T0_PM_C_CREDIT, EVENT_T0_PM_C_GIVE,
        PROGRAM_HASHES, STATE_0, STATE_1,
    };
    use crate::subcircuits::bounded;
    use crate::test_utils::{C, CONFIG, D, F, ZERO_VAL};

    pub struct DummyLeafCircuit {
        pub bounded: bounded::LeafSubCircuit,
        pub state_from_events: LeafSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyLeafCircuit {
        #[must_use]
        pub fn new(circuit_config: &CircuitConfig) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let state_from_events_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_leaf(&mut builder);
            let state_from_events_targets = state_from_events_inputs.build_leaf(&mut builder);

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let state_from_events = state_from_events_targets.build(public_inputs);

            Self {
                bounded,
                state_from_events,
                circuit,
            }
        }

        pub fn prove(&self, event: Event<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.state_from_events.set_witness(&mut inputs, event);
            self.circuit.prove(inputs)
        }

        #[allow(dead_code)]
        fn prove_unsafe(&self, v: LeafWitnessValue<F>) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            self.bounded.set_witness(&mut inputs);
            self.state_from_events.set_witness_unsafe(&mut inputs, v);
            self.circuit.prove(inputs)
        }
    }

    pub struct DummyBranchCircuit {
        pub bounded: bounded::BranchSubCircuit<D>,
        pub state_from_events: BranchSubCircuit,
        pub circuit: CircuitData<F, C, D>,
    }

    impl DummyBranchCircuit {
        #[must_use]
        pub fn new(
            circuit_config: &CircuitConfig,
            indices: &PublicIndices,
            child: &CircuitData<F, C, D>,
        ) -> Self {
            let mut builder = CircuitBuilder::<F, D>::new(circuit_config.clone());

            let bounded_inputs = bounded::SubCircuitInputs::default(&mut builder);
            let state_from_events_inputs = SubCircuitInputs::default(&mut builder);

            let bounded_targets = bounded_inputs.build_branch(&mut builder, child);
            let state_from_events_targets = state_from_events_inputs.build_branch(
                &mut builder,
                indices,
                &bounded_targets.left_proof,
                &bounded_targets.right_proof,
            );

            let circuit = builder.build();

            let public_inputs = &circuit.prover_only.public_inputs;
            let bounded = bounded_targets.build(public_inputs);
            let state_from_events = state_from_events_targets.build(indices, public_inputs);

            Self {
                bounded,
                state_from_events,
                circuit,
            }
        }

        #[must_use]
        pub fn from_leaf(circuit_config: &CircuitConfig, leaf: &DummyLeafCircuit) -> Self {
            Self::new(
                circuit_config,
                &leaf.state_from_events.indices,
                &leaf.circuit,
            )
        }

        #[must_use]
        pub fn from_branch(circuit_config: &CircuitConfig, branch: &Self) -> Self {
            Self::new(
                circuit_config,
                &branch.state_from_events.indices,
                &branch.circuit,
            )
        }

        pub fn prove(
            &self,
            v: BranchWitnessValue<F>,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Option<&ProofWithPublicInputs<F, C, D>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            let partial = right_proof.is_none();
            let right_proof = right_proof.unwrap_or(left_proof);
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            self.state_from_events.set_witness(&mut inputs, partial, v);
            self.circuit.prove(inputs)
        }

        pub fn prove_implicit(
            &self,
            left_proof: &ProofWithPublicInputs<F, C, D>,
            right_proof: Option<&ProofWithPublicInputs<F, C, D>>,
        ) -> Result<ProofWithPublicInputs<F, C, D>> {
            let mut inputs = PartialWitness::new();
            let partial = right_proof.is_none();
            let right_proof = right_proof.unwrap_or(left_proof);
            self.bounded
                .set_witness(&mut inputs, left_proof, right_proof);
            if partial {
                self.state_from_events
                    .set_witness_from_proof(&mut inputs, &left_proof.public_inputs);
            } else {
                self.state_from_events.set_witness_from_proofs(
                    &mut inputs,
                    &left_proof.public_inputs,
                    &right_proof.public_inputs,
                );
            }
            self.circuit.prove(inputs)
        }
    }

    #[tested_fixture::tested_fixture(LEAF)]
    fn build_leaf() -> DummyLeafCircuit { DummyLeafCircuit::new(&CONFIG) }

    #[tested_fixture::tested_fixture(BRANCHES)]
    fn build_branches() -> [DummyBranchCircuit; 2] {
        let b0 = DummyBranchCircuit::from_leaf(&CONFIG, &LEAF);
        let b1 = DummyBranchCircuit::from_branch(&CONFIG, &b0);
        [b0, b1]
    }

    #[allow(clippy::too_many_arguments)]
    fn assert_leaf(
        proof: &ProofWithPublicInputs<F, C, D>,
        address: u64,
        flags: impl Into<BitFlags<EventFlags>>,
        old_owner: impl Into<Option<[F; 4]>>,
        new_owner: impl Into<Option<[F; 4]>>,
        old_data: impl Into<Option<[F; 4]>>,
        new_data: impl Into<Option<[F; 4]>>,
        credit_delta: impl Into<Option<F>>,
    ) {
        let indices = &LEAF.state_from_events.indices;
        let p_address = indices.address.get_field(&proof.public_inputs);
        assert_eq!(p_address, F::from_canonical_u64(address));
        let p_flags = indices.object_flags.get_field(&proof.public_inputs);
        assert_eq!(p_flags, F::from_canonical_u8(flags.into().bits()));
        let p_old_owner = indices.old_owner.get_field(&proof.public_inputs);
        assert_eq!(p_old_owner, old_owner.into().unwrap_or_default());
        let p_new_owner = indices.new_owner.get_field(&proof.public_inputs);
        assert_eq!(p_new_owner, new_owner.into().unwrap_or_default());
        let p_old_data = indices.old_data.get_field(&proof.public_inputs);
        assert_eq!(p_old_data, old_data.into().unwrap_or_default());
        let p_new_data = indices.new_data.get_field(&proof.public_inputs);
        assert_eq!(p_new_data, new_data.into().unwrap_or_default());
        let p_credit_delta = indices.credit_delta.get_field(&proof.public_inputs);
        assert_eq!(p_credit_delta, credit_delta.into().unwrap_or_default());
    }

    #[tested_fixture::tested_fixture(WRITE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_write_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P0_A_WRITE;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            EventFlags::WriteFlag,
            event.owner,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(READ_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_read_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P2_A_READ;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            EventFlags::ReadFlag,
            None,
            None,
            event.value,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(ENSURE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_ensure_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P2_A_ENSURE;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            EventFlags::EnsureFlag,
            None,
            None,
            None,
            event.value,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(GIVE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_give_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_PM_C_GIVE;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            EventFlags::GiveOwnerFlag,
            event.owner,
            event.value,
            None,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(TAKE_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_take_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P2_C_TAKE;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            EventFlags::TakeOwnerFlag,
            event.value,
            event.owner,
            None,
            None,
            None,
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(CREDIT_PLUS_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_credit_plus_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_PM_C_CREDIT;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            BitFlags::empty(),
            event.owner,
            None,
            None,
            None,
            event.value[0],
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(CREDIT_MINUS_LEAF_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_credit_minus_leaf() -> Result<ProofWithPublicInputs<F, C, D>> {
        let event = EVENT_T0_P0_A_CREDIT;
        let proof = LEAF.prove(event)?;
        assert_leaf(
            &proof,
            event.address,
            BitFlags::empty(),
            event.owner,
            None,
            None,
            None,
            -event.value[0],
        );
        LEAF.circuit.verify(proof.clone())?;
        Ok(proof)
    }

    fn leaf_test_helper<Fn>(owner: [F; 4], ty: EventType, value: [u64; 4], f: Fn)
    where
        Fn: FnOnce(&mut LeafWitnessValue<F>, [F; 4], [F; 4]) + UnwindSafe, {
        let event = catch_unwind(|| {
            let value = value.map(F::from_canonical_u64);

            let mut event = LeafWitnessValue::from_event(Event {
                owner,
                ty,
                address: 200,
                value,
            });

            f(&mut event, owner, value);

            event
        })
        .expect("shouldn't fail");

        LEAF.prove_unsafe(event).unwrap();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_1() {
        leaf_test_helper(
            PROGRAM_HASHES[0],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = EventFlags::EnsureFlag.into();
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_2() {
        leaf_test_helper(
            PROGRAM_HASHES[0],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = EventFlags::GiveOwnerFlag.into();
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_3() {
        leaf_test_helper(
            PROGRAM_HASHES[0],
            EventType::Write,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = EventFlags::EnsureFlag | EventFlags::WriteFlag;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_write_leaf_4() {
        leaf_test_helper(
            PROGRAM_HASHES[0],
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
            PROGRAM_HASHES[0],
            EventType::Ensure,
            [3, 1, 4, 15],
            |event, _, _| {
                event.object_flags = EventFlags::WriteFlag.into();
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_give_leaf_1() {
        leaf_test_helper(
            PROGRAM_HASHES[0],
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
            PROGRAM_HASHES[0],
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
            PROGRAM_HASHES[0],
            EventType::CreditDelta,
            [13, 0, 0, 0],
            |event, _, _| {
                event.credit_delta *= -1;
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_credit_leaf_sign() {
        leaf_test_helper(
            PROGRAM_HASHES[0],
            EventType::CreditDelta,
            [13, 0, 0, 1],
            |event, _, _| {
                event.event_value[3] = F::from_canonical_u64(12);
            },
        );
    }

    struct EventData {
        owner: [u64; 4],
        ty: EventType,
        value: [u64; 4],
    }

    struct EventProof<E> {
        event: E,
        proof: ProofWithPublicInputs<F, C, D>,
    }

    trait Constructable: UnwindSafe {
        type Constructed;
        fn construct(self, f: impl Fn(&mut LeafWitnessValue<F>)) -> Self::Constructed;
    }

    impl Constructable for EventData {
        type Constructed = EventProof<LeafWitnessValue<F>>;

        fn construct(self, f: impl Fn(&mut LeafWitnessValue<F>)) -> Self::Constructed {
            let mut event = LeafWitnessValue::from_event(Event {
                address: 200,
                owner: self.owner.map(F::from_canonical_u64),
                ty: self.ty,
                value: self.value.map(F::from_canonical_u64),
            });
            f(&mut event);
            let proof = LEAF.prove_unsafe(event).unwrap();
            EventProof { event, proof }
        }
    }

    impl<T: Constructable> Constructable for (T, T) {
        type Constructed = (T::Constructed, T::Constructed);

        fn construct(self, f: impl Fn(&mut LeafWitnessValue<F>)) -> Self::Constructed {
            (self.0.construct(&f), self.1.construct(f))
        }
    }

    trait Mergeable {
        const HEIGHT: usize;
        fn merge(
            self,
            f: impl Fn(&mut BranchWitnessValue<F>),
        ) -> impl FnOnce() -> EventProof<BranchWitnessValue<F>>;
    }

    impl Mergeable
        for (
            EventProof<LeafWitnessValue<F>>,
            EventProof<LeafWitnessValue<F>>,
        )
    {
        const HEIGHT: usize = 0;

        fn merge(
            self,
            f: impl Fn(&mut BranchWitnessValue<F>),
        ) -> impl FnOnce() -> EventProof<BranchWitnessValue<F>> {
            let mut event = BranchWitnessValue::from_branches(self.0.event, self.1.event);
            f(&mut event);
            move || {
                let proof = BRANCHES[Self::HEIGHT]
                    .prove(event, &self.0.proof, Some(&self.1.proof))
                    .unwrap();
                EventProof { event, proof }
            }
        }
    }

    impl<T: Mergeable> Mergeable for (T, T) {
        const HEIGHT: usize = <T as Mergeable>::HEIGHT + 1;

        fn merge(
            self,
            f: impl Fn(&mut BranchWitnessValue<F>),
        ) -> impl FnOnce() -> EventProof<BranchWitnessValue<F>> {
            let left = self.0.merge(&f)();
            let right = self.1.merge(&f)();

            let mut event = BranchWitnessValue::from_branches(left.event, right.event);
            f(&mut event);
            move || {
                let proof = BRANCHES[Self::HEIGHT]
                    .prove(event, &left.proof, Some(&right.proof))
                    .unwrap();
                EventProof { event, proof }
            }
        }
    }

    fn branch_test_helper<V: Constructable>(
        v: V,
        leaf: impl Fn(&mut LeafWitnessValue<F>) + UnwindSafe,
        branch: impl Fn(&mut BranchWitnessValue<F>) + UnwindSafe,
    ) where
        V::Constructed: Mergeable, {
        let final_branch_merge = catch_unwind(|| {
            let v = v.construct(leaf);
            v.merge(branch)
        })
        .expect("shouldn't fail");

        final_branch_merge();
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_mismatch_address_1() {
        let i = Cell::new(0);
        branch_test_helper(
            (
                (
                    EventData {
                        owner: [4, 8, 15, 16],
                        ty: EventType::Write,
                        value: [3, 1, 4, 15],
                    },
                    EventData {
                        owner: [2, 3, 4, 2],
                        ty: EventType::Read,
                        value: [1, 6, 180, 33],
                    },
                ),
                (
                    EventData {
                        owner: [2, 3, 4, 2],
                        ty: EventType::Ensure,
                        value: [3, 1, 4, 15],
                    },
                    EventData {
                        owner: [2, 3, 4, 2],
                        ty: EventType::Ensure,
                        value: [3, 1, 4, 15],
                    },
                ),
            ),
            move |event| {
                // Alter the address of the last two events
                if matches!(i.get(), 2 | 3) {
                    event.address += 10;
                }
                i.set(i.get() + 1);
            },
            |_: &mut _| {},
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_mismatch_address_2() {
        let i = Cell::new(0);
        branch_test_helper(
            (
                (
                    EventData {
                        owner: [4, 8, 15, 16],
                        ty: EventType::Write,
                        value: [3, 1, 4, 15],
                    },
                    EventData {
                        owner: [2, 3, 4, 2],
                        ty: EventType::Read,
                        value: [1, 6, 180, 33],
                    },
                ),
                (
                    EventData {
                        owner: [2, 3, 4, 2],
                        ty: EventType::Ensure,
                        value: [3, 1, 4, 15],
                    },
                    EventData {
                        owner: [2, 3, 4, 2],
                        ty: EventType::Ensure,
                        value: [3, 1, 4, 15],
                    },
                ),
            ),
            |_| {},
            move |event| {
                // Mess up the final address
                if i.get() == 2 {
                    event.address += 10;
                }
                i.set(i.get() + 1);
            },
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_double_write() {
        branch_test_helper(
            (
                EventData {
                    owner: [4, 8, 15, 16],
                    ty: EventType::Write,
                    value: [3, 1, 4, 15],
                },
                EventData {
                    owner: [4, 8, 15, 16],
                    ty: EventType::Write,
                    value: [3, 1, 4, 15],
                },
            ),
            |_| {},
            |_| {},
        );
    }

    #[test]
    #[should_panic(expected = "was set twice with different values")]
    fn bad_branch_double_credit_sum() {
        branch_test_helper(
            (
                EventData {
                    owner: [4, 8, 15, 16],
                    ty: EventType::CreditDelta,
                    value: [13, 0, 0, 0],
                },
                EventData {
                    owner: [4, 8, 15, 16],
                    ty: EventType::CreditDelta,
                    value: [8, 0, 0, 1],
                },
            ),
            |_| {},
            |event| {
                assert_eq!(event.credit_delta, 5);
                event.credit_delta += 10;
            },
        );
    }

    #[tested_fixture::tested_fixture(READ_WRITE_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_read_write_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let witness = BranchWitnessValue {
            address: ADDRESS_A as u64,
            object_flags: EventFlags::ReadFlag | EventFlags::WriteFlag,
            old_owner: PROGRAM_HASHES[0],
            new_owner: ZERO_VAL,
            old_data: STATE_0[ADDRESS_A].data,
            new_data: STATE_1[ADDRESS_A].data,
            credit_delta: 0,
        };
        let proof = BRANCHES[0].prove(witness, &READ_LEAF_PROOF, Some(&WRITE_LEAF_PROOF))?;
        BRANCHES[0].circuit.verify(proof)?;
        let proof = BRANCHES[0].prove(witness, &WRITE_LEAF_PROOF, Some(&READ_LEAF_PROOF))?;
        BRANCHES[0].circuit.verify(proof)?;
        let proof = BRANCHES[0].prove_implicit(&READ_LEAF_PROOF, Some(&WRITE_LEAF_PROOF))?;
        BRANCHES[0].circuit.verify(proof)?;
        let proof = BRANCHES[0].prove_implicit(&WRITE_LEAF_PROOF, Some(&READ_LEAF_PROOF))?;
        BRANCHES[0].circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[tested_fixture::tested_fixture(ENSURE_BRANCH_PROOF: ProofWithPublicInputs<F, C, D>)]
    fn verify_ensure_branch() -> Result<ProofWithPublicInputs<F, C, D>> {
        let witness = BranchWitnessValue {
            address: ADDRESS_A as u64,
            object_flags: EventFlags::EnsureFlag.into(),
            old_owner: ZERO_VAL,
            new_owner: ZERO_VAL,
            old_data: ZERO_VAL,
            new_data: STATE_1[ADDRESS_A].data,
            credit_delta: 0,
        };
        let proof = BRANCHES[0].prove(witness, &ENSURE_LEAF_PROOF, Some(&ENSURE_LEAF_PROOF))?;
        BRANCHES[0].circuit.verify(proof)?;
        let proof = BRANCHES[0].prove_implicit(&ENSURE_LEAF_PROOF, Some(&ENSURE_LEAF_PROOF))?;
        BRANCHES[0].circuit.verify(proof.clone())?;
        Ok(proof)
    }

    #[test]
    fn verify_left_branches() -> Result<()> {
        let leafs = [
            &WRITE_LEAF_PROOF,
            &READ_LEAF_PROOF,
            &ENSURE_LEAF_PROOF,
            &GIVE_LEAF_PROOF,
            &TAKE_LEAF_PROOF,
            &CREDIT_PLUS_LEAF_PROOF,
            &CREDIT_MINUS_LEAF_PROOF,
        ];
        for leaf in leafs {
            let proof = BRANCHES[0].prove_implicit(leaf, None)?;
            BRANCHES[0].circuit.verify(proof)?;
        }
        Ok(())
    }

    #[test]
    fn verify_left_double_branch() -> Result<()> {
        let branches = [&READ_WRITE_BRANCH_PROOF, &ENSURE_BRANCH_PROOF];
        for branch in branches {
            let proof = BRANCHES[1].prove_implicit(branch, None)?;
            BRANCHES[1].circuit.verify(proof)?;
        }
        Ok(())
    }

    #[test]
    fn verify_double_branch() -> Result<()> {
        let witness = BranchWitnessValue {
            address: ADDRESS_A as u64,
            object_flags: EventFlags::ReadFlag | EventFlags::WriteFlag | EventFlags::EnsureFlag,
            old_owner: PROGRAM_HASHES[0],
            new_owner: ZERO_VAL,
            old_data: STATE_0[ADDRESS_A].data,
            new_data: STATE_1[ADDRESS_A].data,
            credit_delta: 0,
        };

        let proof = BRANCHES[1].prove(
            witness,
            &READ_WRITE_BRANCH_PROOF,
            Some(&ENSURE_BRANCH_PROOF),
        )?;
        BRANCHES[1].circuit.verify(proof)?;
        let proof = BRANCHES[1].prove(
            witness,
            &ENSURE_BRANCH_PROOF,
            Some(&READ_WRITE_BRANCH_PROOF),
        )?;
        BRANCHES[1].circuit.verify(proof)?;

        let proof =
            BRANCHES[1].prove_implicit(&READ_WRITE_BRANCH_PROOF, Some(&ENSURE_BRANCH_PROOF))?;
        BRANCHES[1].circuit.verify(proof)?;
        let proof =
            BRANCHES[1].prove_implicit(&ENSURE_BRANCH_PROOF, Some(&READ_WRITE_BRANCH_PROOF))?;
        BRANCHES[1].circuit.verify(proof)?;

        Ok(())
    }
}
