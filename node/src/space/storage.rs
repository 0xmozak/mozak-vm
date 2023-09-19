use std::collections::HashMap;

use crate::space::object::program::ProgramContent;
use crate::space::object::Object::Program;
use crate::space::object::{Object, TransitionFunction};
use crate::Id;

/// Id-focused storage for the application space.
pub struct ApplicationStorage {
    objects: HashMap<Id, Object>,
}

impl ApplicationStorage {
    /// Initiates a new storage.
    /// We always add the genesis object to the storage.
    /// This object is a program that allows any transition.
    /// It is up to the user to then add more programs to the storage.
    /// This is the only object that is self-owned.
    pub fn initiate() -> Self {
        // Transition that allows any transition.
        // TODO - pass it a real transition.
        let yes_man_transition = TransitionFunction::default();

        let genesis_object = Program(ProgramContent::new(0, true, Id::default(), vec![
            yes_man_transition,
        ]));

        let mut storage = ApplicationStorage::new();

        storage.update_objects(vec![genesis_object]);

        storage
    }

    /// Creates a new empty storage.
    fn new() -> Self {
        ApplicationStorage {
            objects: HashMap::new(),
        }
    }

    /// Returns an object by its id. If the object does not exist, then we
    /// return None.
    pub fn get_object(&self, id: Id) -> Option<&Object> { self.objects.get(&id) }

    /// Adds a list of objects to the storage. If an object with the same id
    /// already exists, then the update replaces it with the new one.
    ///
    /// We will be pushing updates to the network as a list of changed objects,
    /// as well as a proof that the state transition of the blobs is valid.
    /// Hence we will use the batch update method.
    pub fn update_objects(&mut self, objects: Vec<Object>) {
        for object in objects {
            self.objects.insert(object.id(), object);
        }
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches::assert_matches;

    use super::*;
    use crate::space::object::data::DataContent;
    use crate::space::object::ObjectContent;

    #[test]
    fn test_storage() {
        let mut storage = ApplicationStorage::initiate();

        let owner = Id::default();

        let (is_executable, owner_id, id_parameters, data) =
            (false, Id::default(), Vec::<&[u8]>::new(), vec![
                1, 2, 3, 4, 5,
            ]);

        let object = DataContent::new(true, owner_id, vec![1u8]);
        let object_id = object.id();
        let wrapped_object = Object::Data(object);

        storage.update_objects(vec![wrapped_object]);

        let retrieved_object = storage.get_object(object_id).unwrap();

        // Check that all object parameters are the same.
        assert_matches!(retrieved_object, Object::Data(object));
    }

    #[test]
    fn test_that_blobs_with_same_id_replace_each_other() {
        let mut storage = ApplicationStorage::initiate();

        let owner = Id::default();

        let object = DataContent::new(true, owner, vec![1u8]);
        let object_id = object.id();

        let wrapped_object = Object::Data(object.clone());

        storage.update_objects(vec![wrapped_object]);

        let updated_object = object.transition(vec![2u8]);
        let wrapped_updated_object = Object::Data(updated_object);

        storage.update_objects(vec![wrapped_updated_object]);

        let retrieved_object = storage.get_object(object_id).unwrap();

        assert_matches!(retrieved_object, Object::Data(updated_object));
    }
}
