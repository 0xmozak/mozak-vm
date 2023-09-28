#![cfg_attr(target_os = "zkvm", feature(restricted_std))]
#![cfg_attr(target_os = "zkvm", no_main)]

use examples::{setup_main, Transition};
use mozak_node_sdk::TransitionInput;

struct AtLeastOneNewObject;

impl Transition for AtLeastOneNewObject {
    fn validate(transition_input: TransitionInput) -> bool {
        // Yes Man always returns true.
        transition_input.changed_objects_after.len() > 0
    }
}

setup_main!(AtLeastOneNewObject);

#[cfg(all(test, not(target_os = "zkvm")))]
mod test {
    use examples::Transition;
    use mozak_node_sdk::{Object, TransitionInput};

    use crate::AtLeastOneNewObject;

    #[test]
    fn test_validation_fails_when_no_objects() {
        let new_object = TransitionInput::default();

        assert_eq!(AtLeastOneNewObject::validate(new_object), false);
    }

    #[test]
    fn test_validation_succeeds_when_one_object() {
        let new_object = TransitionInput {
            changed_objects_after: vec![Object::default()],
            ..TransitionInput::default()
        };

        assert_eq!(AtLeastOneNewObject::validate(new_object), true);
    }
}
