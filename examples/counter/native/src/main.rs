use counter_core_logic::{dispatch, Counter, MethodArgs, MethodReturns};
use counter_elf_data::COUNTER_SELF_PROG_ID;
use mozak_sdk::common::types::{ProgramIdentifier, StateAddress, StateObject};
use rkyv::rancor::Panic;

fn main() {
    let counter_program = ProgramIdentifier::from(COUNTER_SELF_PROG_ID.to_string());
    let address = StateAddress::new_from_rand_seed(1);
    let counter = Counter::new(10);

    // can be retrieved from public oracle
    let state_object = StateObject {
        address,
        constraint_owner: counter_program,
        data: rkyv::to_bytes::<_, 256, Panic>(&counter).unwrap().to_vec(),
    };

    let new_object1 = if let MethodReturns::IncreaseCounter(new_object1) = mozak_sdk::call_send(
        counter_program,
        MethodArgs::IncreaseCounter(state_object.clone()),
        dispatch,
    ) {
        new_object1
    } else {
        unreachable!()
    };

    let new_object2 = if let MethodReturns::IncreaseCounter(new_object2) = mozak_sdk::call_send(
        counter_program,
        MethodArgs::IncreaseCounter(new_object1),
        dispatch,
    ) {
        new_object2
    } else {
        unreachable!()
    };

    let new_object3 = if let MethodReturns::DecreaseCounter(new_object2) = mozak_sdk::call_send(
        counter_program,
        MethodArgs::DecreaseCounter(new_object2),
        dispatch,
    ) {
        new_object2
    } else {
        unreachable!()
    };

    let old_counter = unsafe { rkyv::access_unchecked::<Counter>(&state_object.data) };
    let new_counter = unsafe { rkyv::access_unchecked::<Counter>(&new_object3.data) };

    assert_eq!(old_counter.0 + 1, new_counter.0);

    mozak_sdk::native::dump_proving_files();
}
