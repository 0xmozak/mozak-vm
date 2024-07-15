use counter_core_logic::{dispatch, Counter, MethodArgs};
use counter_elf_data::COUNTER_SELF_PROG_ID;
use mozak_sdk::common::types::{ProgramIdentifier, StateAddress, StateObject};
use rkyv::rancor::Panic;

fn main() {
    let counter_program = ProgramIdentifier::from(COUNTER_SELF_PROG_ID.to_string());
    let address = StateAddress::new_from_rand_seed(1);

    // data below can be retrieved from public orace. Hardcoding it for the moment.
    let counter = Counter(10);
    let state_object = StateObject {
        address,
        constraint_owner: counter_program,
        data: rkyv::to_bytes::<_, 256, Panic>(&counter).unwrap().to_vec(),
    };

    // increase counter by 1
    let new_object1: StateObject = mozak_sdk::call_send(
        counter_program,
        MethodArgs::IncreaseCounter(state_object.clone()),
        dispatch,
    )
    .into();

    // increase counter by 1
    let new_object2: StateObject = mozak_sdk::call_send(
        counter_program,
        MethodArgs::IncreaseCounter(new_object1),
        dispatch,
    )
    .into();

    let counter = unsafe { rkyv::access_unchecked::<Counter>(&new_object2.data) };
    println!("Counter State after two increments: {}", counter.0);

    // decrease counter by 1
    let new_object3: StateObject = mozak_sdk::call_send(
        counter_program,
        MethodArgs::DecreaseCounter(new_object2),
        dispatch,
    )
    .into();

    let counter = unsafe { rkyv::access_unchecked::<Counter>(&new_object3.data) };
    println!("Counter state after decrement: {}", counter.0);

    // `rkyv::access`` the `data` field of state objects as `Counter`, to compare
    // them without extra deserialization
    let old_counter = unsafe { rkyv::access_unchecked::<Counter>(&state_object.data) };
    let new_counter = unsafe { rkyv::access_unchecked::<Counter>(&new_object3.data) };

    // check that counter was updated correctly.
    assert_eq!(old_counter.0 + 1, new_counter.0);

    mozak_sdk::native::dump_proving_files();
}
