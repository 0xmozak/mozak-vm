use std::collections::HashMap;

use mozak_node_sdk::{Id, Object};

/// Id-focused storage for the application network.
pub struct ApplicationStorage {
    objects: HashMap<Id, Object>,
}

impl ApplicationStorage {
    /// Initiates a new storage.
    /// We always add the genesis object to the storage.
    /// This object is a program that allows any transition.
    /// It is up to the user to then add more programs to the storage.
    /// This is the only object that is self-owned.
    pub fn initiate(root_object: Object) -> Self {
        let mut storage = ApplicationStorage::new();

        storage.update_objects(vec![root_object]);

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
    use mozak_node_sdk::data::DataContent;
    use mozak_node_sdk::ObjectContent;

    use super::*;

    #[test]
    fn test_storage() {
        let root_object = Object::default();

        let mut storage = ApplicationStorage::initiate(root_object);

        let owner_id = Id::default();

        let object = DataContent::new(true, owner_id, vec![1u8]);
        let object_id = object.id();
        let wrapped_object = Object::Data(object);

        storage.update_objects(vec![wrapped_object]);

        let retrieved_object = storage.get_object(object_id).unwrap();

        match retrieved_object {
            Object::Program(_) => panic!("Expected a Data object"),
            Object::Data(ret_object_content) =>
                if object_id == ret_object_content.id() { // pass
                } else {
                    panic!("Expected the same object")
                },
        }
    }

    #[test]
    fn test_that_blobs_with_same_id_replace_each_other() {
        let root_object = Object::default();

        let mut storage = ApplicationStorage::initiate(root_object);

        let owner = Id::default();

        let object = DataContent::new(true, owner, vec![1u8]);
        let object_id = object.id();

        let wrapped_object = Object::Data(object.clone());

        storage.update_objects(vec![wrapped_object]);

        let new_data = vec![2u8];
        let updated_object = object.transition(new_data.clone());
        let wrapped_updated_object = Object::Data(updated_object);

        storage.update_objects(vec![wrapped_updated_object]);

        let retrieved_object = storage.get_object(object_id).unwrap();

        match retrieved_object {
            Object::Program(_) => panic!("Expected a Data object"),
            Object::Data(ret_object_content) =>
                if object_id == ret_object_content.id() && new_data == ret_object_content.data { // pass
                } else {
                    panic!("Expected the same object")
                },
        }
    }
}
