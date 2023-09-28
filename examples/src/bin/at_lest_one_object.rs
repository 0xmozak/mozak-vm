#![no_main]
#![no_std]

use examples::Transition;
use mozak_node_sdk::TransitionInput;

struct AtLeastOneNewObject;

impl Transition for AtLeastOneNewObject {
    fn validate(transition_input: TransitionInput) -> bool {
        // Yes Man always returns true.
        transition_input.changed_objects_after.len() > 0
    }
}

guest::entry!(AtLeastOneNewObject::run);
