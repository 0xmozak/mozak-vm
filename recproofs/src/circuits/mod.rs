pub mod accumulate_delta;
pub mod build_event_root;
pub mod match_delta;
pub mod merge;
pub mod state_update;
pub mod verify_program;
pub mod verify_tx;

/// A repository of testing data to allow unit tests to build on one another
/// and cross-reference by having them all draw from a consistent transaction
/// set.
///
/// At present this consists of 2 transactions modifying a state-tree of size 8
/// (only addresses 0..=7 are valid).
#[cfg(test)]
pub mod test_data {
    use once_cell::sync::Lazy;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::HashOut;

    use crate::test_utils::{
        hash_branch, hash_val_bytes, make_f, make_fs, F, NON_ZERO_VALUES, ZERO_VAL,
    };
    use crate::{Event, EventType, Object};

    /// The hashes of the programs used
    pub const PROGRAM_HASHES: [[F; 4]; 3] = [
        make_fs([31, 41, 59, 26]),
        make_fs([53, 58, 97, 93]),
        make_fs([23, 84, 62, 64]),
    ];

    /// Each transaction has a call list
    pub const CALL_LISTS: [[F; 4]; 2] = [make_fs([86, 7, 5, 309]), make_fs([8, 67, 530, 9])];

    /// Each transaction has a call list
    pub static CAST_PM_P0: Lazy<[F; 4]> = Lazy::new(|| hash_val_bytes(ZERO_VAL, PROGRAM_HASHES[0]));
    pub static CAST_T0: Lazy<[F; 4]> = Lazy::new(|| hash_val_bytes(*CAST_PM_P0, PROGRAM_HASHES[2]));
    pub static CAST_PM_P1: Lazy<[F; 4]> = Lazy::new(|| hash_val_bytes(ZERO_VAL, PROGRAM_HASHES[1]));
    pub static CAST_T1: Lazy<[F; 4]> = Lazy::new(|| hash_val_bytes(*CAST_PM_P1, PROGRAM_HASHES[2]));
    pub static CAST_ROOT: Lazy<[HashOut<F>; 2]> =
        Lazy::new(|| [HashOut::from(*CAST_T0), HashOut::from(*CAST_T1)]);

    // The addresses that will be used by events
    /// An address being updated
    pub const ADDRESS_A: usize = 2;
    /// An address being deleted
    pub const ADDRESS_B: usize = 4;
    /// An address being created
    pub const ADDRESS_C: usize = 5;
    /// An address being read
    pub const ADDRESS_D: usize = 6;
    /// An address being ignored
    pub const ADDRESS_E: usize = 7;

    /// The empty object
    pub const ZERO_OBJ: Object<F> = Object {
        constraint_owner: ZERO_VAL,
        last_updated: F::ZERO,
        credits: F::ZERO,
        data: ZERO_VAL,
    };

    /// The hash of the empty object
    pub static ZERO_OBJ_HASH: Lazy<HashOut<F>> = Lazy::new(|| ZERO_OBJ.hash());

    /// The initial state
    pub const STATE_0: [Object<F>; 8] = {
        let mut state = [ZERO_OBJ; 8];

        state[ADDRESS_A] = Object {
            constraint_owner: PROGRAM_HASHES[0],
            last_updated: F::ZERO,
            credits: make_f(400),
            data: ZERO_VAL,
        };
        state[ADDRESS_B] = Object {
            constraint_owner: PROGRAM_HASHES[1],
            last_updated: F::ZERO,
            credits: make_f(500),
            data: NON_ZERO_VALUES[0],
        };
        state[ADDRESS_D] = Object {
            constraint_owner: PROGRAM_HASHES[1],
            last_updated: F::ZERO,
            credits: F::ZERO,
            data: NON_ZERO_VALUES[1],
        };
        state[ADDRESS_E] = Object {
            constraint_owner: PROGRAM_HASHES[2],
            last_updated: F::ZERO,
            credits: F::ZERO,
            data: NON_ZERO_VALUES[2],
        };

        state
    };

    /// The next state
    pub const STATE_1: [Object<F>; 8] = {
        let mut state = [ZERO_OBJ; 8];

        state[ADDRESS_A] = Object {
            constraint_owner: PROGRAM_HASHES[0],
            last_updated: make_f(1),
            credits: make_f(100),
            data: NON_ZERO_VALUES[3],
        };
        state[ADDRESS_C] = Object {
            constraint_owner: PROGRAM_HASHES[2],
            last_updated: make_f(1),
            credits: make_f(300),
            data: NON_ZERO_VALUES[4],
        };
        state[ADDRESS_D] = STATE_0[ADDRESS_D];
        state[ADDRESS_E] = STATE_0[ADDRESS_E];

        state
    };

    // The events of the first transaction

    pub const EVENT_T0_PM_C_CREDIT: Event<F> = Event {
        address: ADDRESS_C as u64,
        owner: [F::ZERO; 4],
        ty: EventType::CreditDelta,
        value: make_fs([300, 0, 0, 0]),
    };

    pub const EVENT_T0_PM_C_GIVE: Event<F> = Event {
        address: ADDRESS_C as u64,
        owner: [F::ZERO; 4],
        ty: EventType::GiveOwner,
        value: PROGRAM_HASHES[2],
    };

    pub const EVENT_T0_PM_C_WRITE: Event<F> = Event {
        address: ADDRESS_C as u64,
        owner: [F::ZERO; 4],
        ty: EventType::Write,
        value: NON_ZERO_VALUES[4],
    };

    pub const EVENT_T0_P0_A_WRITE: Event<F> = Event {
        address: ADDRESS_A as u64,
        owner: PROGRAM_HASHES[0],
        ty: EventType::Write,
        value: NON_ZERO_VALUES[3],
    };

    pub const EVENT_T0_P0_A_CREDIT: Event<F> = Event {
        address: ADDRESS_A as u64,
        owner: PROGRAM_HASHES[0],
        ty: EventType::CreditDelta,
        value: make_fs([300, 0, 0, 1]),
    };

    pub const EVENT_T0_P2_A_READ: Event<F> = Event {
        address: ADDRESS_A as u64,
        owner: PROGRAM_HASHES[2],
        ty: EventType::Read,
        value: ZERO_VAL,
    };

    pub const EVENT_T0_P2_A_ENSURE: Event<F> = Event {
        address: ADDRESS_A as u64,
        owner: PROGRAM_HASHES[2],
        ty: EventType::Ensure,
        value: NON_ZERO_VALUES[3],
    };

    pub const EVENT_T0_P2_C_TAKE: Event<F> = Event {
        address: ADDRESS_C as u64,
        owner: PROGRAM_HASHES[2],
        ty: EventType::TakeOwner,
        value: [F::ZERO; 4],
    };

    // Transaction merges
    pub static T0_PM_HASH: Lazy<HashOut<F>> = Lazy::new(|| {
        hash_branch(
            &hash_branch(&EVENT_T0_PM_C_CREDIT.hash(), &EVENT_T0_PM_C_GIVE.hash()),
            &EVENT_T0_PM_C_WRITE.hash(),
        )
    });
    pub static T0_P0_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&EVENT_T0_P0_A_WRITE.hash(), &EVENT_T0_P0_A_CREDIT.hash()));
    pub static T0_PM_P0_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T0_P0_HASH, &T0_PM_HASH));
    pub static T0_P2_A_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&EVENT_T0_P2_A_READ.hash(), &EVENT_T0_P2_A_ENSURE.hash()));
    pub static T0_P2_C_HASH: Lazy<HashOut<F>> = Lazy::new(|| EVENT_T0_P2_C_TAKE.hash());
    pub static T0_P2_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T0_P2_A_HASH, &T0_P2_C_HASH));
    pub static T0_A_HASH: Lazy<HashOut<F>> = Lazy::new(|| hash_branch(&T0_P0_HASH, &T0_P2_A_HASH));
    pub static T0_C_HASH: Lazy<HashOut<F>> = Lazy::new(|| hash_branch(&T0_PM_HASH, &T0_P2_C_HASH));
    pub static T0_HASH: Lazy<HashOut<F>> = Lazy::new(|| hash_branch(&T0_A_HASH, &T0_C_HASH));

    // The events of the second transaction

    pub const EVENT_T1_PM_B_TAKE: Event<F> = Event {
        address: ADDRESS_B as u64,
        owner: [F::ZERO; 4],
        ty: EventType::TakeOwner,
        value: PROGRAM_HASHES[1],
    };

    pub const EVENT_T1_PM_B_ENSURE: Event<F> = Event {
        address: ADDRESS_B as u64,
        owner: [F::ZERO; 4],
        ty: EventType::Ensure,
        value: [F::ZERO; 4],
    };

    pub const EVENT_T1_P1_B_WRITE: Event<F> = Event {
        address: ADDRESS_B as u64,
        owner: PROGRAM_HASHES[1],
        ty: EventType::Write,
        value: ZERO_VAL,
    };

    pub const EVENT_T1_P1_B_GIVE: Event<F> = Event {
        address: ADDRESS_B as u64,
        owner: PROGRAM_HASHES[1],
        ty: EventType::GiveOwner,
        value: ZERO_VAL,
    };

    pub const EVENT_T1_P1_B_CREDIT: Event<F> = Event {
        address: ADDRESS_B as u64,
        owner: PROGRAM_HASHES[1],
        ty: EventType::CreditDelta,
        value: make_fs([500, 0, 0, 1]),
    };

    pub const EVENT_T1_P2_A_READ: Event<F> = Event {
        address: ADDRESS_A as u64,
        owner: PROGRAM_HASHES[2],
        ty: EventType::Read,
        value: ZERO_VAL,
    };

    pub const EVENT_T1_P2_D_READ: Event<F> = Event {
        address: ADDRESS_D as u64,
        owner: PROGRAM_HASHES[2],
        ty: EventType::Read,
        value: NON_ZERO_VALUES[1],
    };

    // Transaction merges
    pub static T1_PM_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&EVENT_T1_PM_B_TAKE.hash(), &EVENT_T1_PM_B_ENSURE.hash()));
    pub static T1_P1_HASH: Lazy<HashOut<F>> = Lazy::new(|| {
        hash_branch(
            &hash_branch(&EVENT_T1_P1_B_WRITE.hash(), &EVENT_T1_P1_B_GIVE.hash()),
            &EVENT_T1_P1_B_CREDIT.hash(),
        )
    });
    pub static T1_B_HASH: Lazy<HashOut<F>> = Lazy::new(|| hash_branch(&T1_PM_HASH, &T1_P1_HASH));
    pub static T1_P2_A_HASH: Lazy<HashOut<F>> = Lazy::new(|| EVENT_T1_P2_A_READ.hash());
    pub static T1_P2_D_HASH: Lazy<HashOut<F>> = Lazy::new(|| EVENT_T1_P2_D_READ.hash());
    pub static T1_P2_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T1_P2_A_HASH, &T1_P2_D_HASH));
    pub static T1_AB_HASH: Lazy<HashOut<F>> = Lazy::new(|| hash_branch(&T1_P2_A_HASH, &T1_B_HASH));
    pub static T1_HASH: Lazy<HashOut<F>> = Lazy::new(|| hash_branch(&T1_AB_HASH, &T1_P2_D_HASH));

    // Cross transaction merges
    pub static T0_T1_A_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T0_A_HASH, &T1_P2_A_HASH));
    pub static T0_T1_AB_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T0_T1_A_HASH, &T1_B_HASH));
    pub static T0_T1_CD_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T0_C_HASH, &T1_P2_D_HASH));
    pub static T0_T1_HASH: Lazy<HashOut<F>> =
        Lazy::new(|| hash_branch(&T0_T1_AB_HASH, &T0_T1_CD_HASH));
}
