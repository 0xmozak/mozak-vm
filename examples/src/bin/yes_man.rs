#![cfg_attr(target_os = "zkvm", feature(restricted_std))]
#![cfg_attr(target_os = "zkvm", no_main)]

use examples::{setup_main, Transition};
use mozak_node_sdk::TransitionInput;

struct YesManTransition;

impl Transition for YesManTransition {
    #[allow(unused_variables)]
    fn validate(transition_input: TransitionInput) -> bool {
        // Yes Man always returns true.
        true
    }
}

setup_main!(YesManTransition);

#[cfg(all(test, not(target_os = "zkvm")))]
mod test {
    use examples::Transition;
    use mozak_node_sdk::TransitionInput;

    use crate::YesManTransition;

    #[test]
    fn test_validation() {
        let new_object = TransitionInput::default();

        assert!(YesManTransition::validate(new_object));
    }
}
