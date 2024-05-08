pub mod accumulate_delta;
pub mod build_event_root;
pub mod match_delta;
pub mod merge;
pub mod state_update;
pub mod verify_block;
pub mod verify_program;
pub mod verify_tx;

#[cfg(test)]
pub mod test_data {
    use once_cell::sync::Lazy;
    use plonky2::field::types::Field;
    use plonky2::hash::hash_types::HashOut;

    use crate::test_utils::{hash_val_bytes, make_f, make_fs, F, NON_ZERO_VALUES, ZERO_VAL};
    use crate::{Event, EventType, Object};

    pub const PROGRAM_HASHES: [[F; 4]; 3] = [
        make_fs([31, 41, 59, 26]),
        make_fs([53, 58, 97, 93]),
        make_fs([23, 84, 62, 64]),
    ];

    /// Each transaction has a call list
    pub const CALL_LISTS: [[F; 4]; 2] = [make_fs([86, 7, 5, 309]), make_fs([8, 67, 530, 9])];

    /// Each transaction has a call list
    pub static CAST_ROOT: Lazy<[HashOut<F>; 2]> = Lazy::new(|| {
        [
            hash_val_bytes(
                hash_val_bytes(ZERO_VAL, PROGRAM_HASHES[0]),
                PROGRAM_HASHES[2],
            )
            .into(),
            hash_val_bytes(
                hash_val_bytes(ZERO_VAL, PROGRAM_HASHES[1]),
                PROGRAM_HASHES[2],
            )
            .into(),
        ]
    });

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

    pub const ZERO_OBJ: Object<F> = Object {
        constraint_owner: ZERO_VAL,
        last_updated: F::ZERO,
        credits: F::ZERO,
        data: ZERO_VAL,
    };

    pub static ZERO_OBJ_HASH: Lazy<HashOut<F>> = Lazy::new(|| ZERO_OBJ.hash());

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
}
