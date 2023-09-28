use mozak_node_sdk::{Id, Object};
use rand::distributions::Standard;
use rand::prelude::Distribution;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Message that contains all the information needed to verify a Program Storage
/// transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionMessage {
    /// The program that defines the transition that is being called
    pub owner_program_id: Id,
    /// The transition that is being called
    pub target_transition_id: Id,
    /// The objects that are being read by the transition
    pub read_objects_id: Vec<Id>,
    /// The objects that are being changed by the transition
    pub changed_objects: Vec<Object>,
    /// The inputs to the transition, represented as a serialised byte array
    pub input: Vec<u8>,
}

#[cfg(feature = "dummy-system")]
impl Distribution<TransitionMessage> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> TransitionMessage {
        TransitionMessage {
            owner_program_id: rng.gen(),
            target_transition_id: rng.gen(),
            read_objects_id: vec![rng.gen(); 10],
            changed_objects: vec![],
            input: vec![],
        }
    }
}

#[cfg(all(test, feature = "dummy-system"))]
mod test {
    use flexbuffers::FlexbufferSerializer;

    use super::*;

    #[test]
    fn test_serialisation_deserialization() {
        let mut rng = rand::thread_rng();

        let message: TransitionMessage = rng.gen();

        let mut serializer = FlexbufferSerializer::new();
        message.serialize(&mut serializer).unwrap();
        let serialized_message = serializer.view();

        let deserialized_message: TransitionMessage =
            flexbuffers::from_slice(serialized_message).unwrap();

        assert_eq!(
            message.read_objects_id,
            deserialized_message.read_objects_id
        );
        assert_eq!(
            message.changed_objects,
            deserialized_message.changed_objects
        );
        assert_eq!(message.input, deserialized_message.input);
        assert_eq!(
            message.owner_program_id,
            deserialized_message.owner_program_id
        );
        assert_eq!(
            message.target_transition_id,
            deserialized_message.target_transition_id
        );
    }
}
