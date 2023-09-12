use std::collections::HashMap;

use crate::space::blobs::Blob;
use crate::Id;

/// Stores a collection of the space blobs.
/// We index them by their id.
pub struct SpaceStorage {
    blobs: HashMap<Id, Blob>,
}

impl SpaceStorage {
    /// Initiates a new empty storage.
    pub fn initiate() -> Self {
        SpaceStorage {
            blobs: HashMap::new(),
        }
    }

    /// Returns a blob by its id.
    pub fn get_blob(&self, id: Id) -> Option<&Blob> { self.blobs.get(&id) }

    /// Updates a list of blobs in the storage. If a blob with the same id
    /// already exists, then we update replace it with the new one.
    ///
    /// We will be pushing updates to the network as a list of chnged blobs, as
    /// well as a proof that the state transition of the blobs is valid. Hence
    /// we will use the batch update method.
    pub fn update_blobs(&mut self, blobs: Vec<Blob>) {
        for blob in blobs {
            self.blobs.insert(blob.id().clone(), blob);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::space::blobs::BlobKind;

    #[test]
    fn test_space_storage() {
        let mut storage = SpaceStorage::initiate();

        let (kind, owner, id_parameters, data) =
            (BlobKind::Data, Id::default(), Vec::<&[u8]>::new(), vec![
                1, 2, 3, 4, 5,
            ]);

        let blob = Blob::new(kind, owner, id_parameters, data);
        let blob_id = *blob.id();
        let parsed_data = blob.data().clone();
        storage.update_blobs(vec![blob]);

        let retrieved_blob = storage.get_blob(blob_id).unwrap();

        // Check that all blob parameters are the same.
        assert_eq!(blob_id, *retrieved_blob.id());
        assert_eq!(kind, retrieved_blob.kind);
        assert_eq!(owner, retrieved_blob.owner);
        assert_eq!(&parsed_data, retrieved_blob.data());
    }

    #[test]
    fn test_that_blobs_with_same_id_replace_each_other() {
        let mut storage = SpaceStorage::initiate();

        let (kind, owner, id_parameters, data) =
            (BlobKind::Data, Id::default(), Vec::<&[u8]>::new(), vec![
                1, 2, 3, 4, 5,
            ]);

        let blob = Blob::new(kind, owner, id_parameters, data);
        let blob_id = *blob.id();
        storage.update_blobs(vec![blob]);

        let (new_kind, new_owner, new_id_parameters, new_data) =
            (BlobKind::Data, Id::default(), Vec::<&[u8]>::new(), vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9,
            ]);

        let updated_blob = Blob::new(new_kind, new_owner, new_id_parameters, new_data);
        let new_parsed_data = updated_blob.data().clone();
        assert_eq!(blob_id, *updated_blob.id());
        storage.update_blobs(vec![updated_blob]);

        let retrieved_blob = storage.get_blob(blob_id).unwrap();

        // Check that all blob parameters are the same.
        assert_eq!(blob_id, *retrieved_blob.id());
        assert_eq!(new_kind, retrieved_blob.kind);
        assert_eq!(new_owner, retrieved_blob.owner);
        assert_eq!(&new_parsed_data, retrieved_blob.data());
    }
}
