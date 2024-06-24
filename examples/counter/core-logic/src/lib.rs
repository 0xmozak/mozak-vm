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
        unsafe { rkyv::access_unchecked::<Counter>(&object.data) }
    }
}

impl Counter {
    pub fn new(value: u64) -> Self { Self(value) }

    pub fn inner(&self) -> u64 { self.0 }

    pub fn increase(&mut self) { self.0 += 1; }

    pub fn decrease(&mut self) { self.0 -= 1; }
}

pub enum CounterMutation {
    Increase,
    Decrease,
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
            // investigate better panic errors like assert?
            _ => panic!(),
        }
    }
}

#[allow(dead_code)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::IncreaseCounter(object) => {
            let new_object = mutate_counter(object, CounterMutation::Increase);
            MethodReturns::IncreaseCounter(new_object)
        }
        MethodArgs::DecreaseCounter(object) => {
            let new_object = mutate_counter(object, CounterMutation::Decrease);
            MethodReturns::DecreaseCounter(new_object)
        }
    }
}

#[allow(dead_code)]
pub fn mutate_counter(state_object: StateObject, mutation: CounterMutation) -> StateObject {
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
    match mutation {
        CounterMutation::Increase => new_counter.increase(),
        CounterMutation::Decrease => new_counter.decrease(),
    }
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
