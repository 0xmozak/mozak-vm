#![no_main]
#![no_std]

use examples::Transition;
use mozak_node_sdk::TransitionInput;

struct YesManTransition;

impl Transition for YesManTransition {
    #[allow(unused_variables)]
    fn validate(transition_input: TransitionInput) -> bool {
        // Yes Man always returns true.
        true
    }
}

guest::entry!(YesManTransition::run);
