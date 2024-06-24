#![feature(restricted_std)]
extern crate alloc;

use core::panic;

use mozak_sdk::common::types::{Event, EventType, StateObject};
use rkyv::rancor::{Panic, Strategy};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub struct Counter(pub u64);

impl<'a> From<&'a StateObject> for &'a ArchivedCounter {
    fn from(object: &'a StateObject) -> Self {
        // TODO: use `rkyv::access` once it is stable
        unsafe { rkyv::access_unchecked::<Counter>(&object.data) }
    }
}

#[derive(Archive, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    IncreaseCounter(StateObject),
    DecreaseCounter(StateObject),
}

#[derive(Archive, Default, Deserialize, Serialize, PartialEq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodReturns {
    #[default]
    Default,
    IncreaseCounter(StateObject),
    DecreaseCounter(StateObject),
}

impl From<MethodReturns> for StateObject {
    fn from(value: MethodReturns) -> Self {
        match value {
            MethodReturns::IncreaseCounter(object) => object,
            MethodReturns::DecreaseCounter(object) => object,
            _ => panic!(),
        }
    }
}

#[allow(dead_code)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::IncreaseCounter(object) => {
            let new_object = mutate_counter(object, 1);
            MethodReturns::IncreaseCounter(new_object)
        }
        MethodArgs::DecreaseCounter(object) => {
            let new_object = mutate_counter(object, -1);
            MethodReturns::DecreaseCounter(new_object)
        }
    }
}

#[allow(dead_code)]
pub fn mutate_counter(state_object: StateObject, delta: i64) -> StateObject {
    let read_event = Event {
        object: state_object.clone(),
        type_: EventType::Read,
    };
    mozak_sdk::event_emit(read_event);
    let archived_counter: &ArchivedCounter = (&state_object).into();
    let counter: Counter = archived_counter
        .deserialize(Strategy::<_, Panic>::wrap(&mut ()))
        .unwrap();
    let mut new_counter = counter.clone();
    new_counter.0 = new_counter.0.checked_add_signed(delta).unwrap();
    let new_state_object = StateObject {
        data: rkyv::to_bytes::<_, 256, Panic>(&new_counter)
            .unwrap()
            .to_vec(),
        ..state_object
    };
    let write_event = Event {
        object: new_state_object.clone(),
        type_: EventType::Write,
    };
    mozak_sdk::event_emit(write_event);
    new_state_object
}
