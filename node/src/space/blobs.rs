use std::ops::Deref;

use sha3::digest::FixedOutput;
use sha3::Digest;

use crate::Id;

/// A struct that represents a single data blob on the network. This can either
/// be a program, or just a stored data. This is analogous to a regular File in
/// computer system.
#[derive(Debug)]
pub struct Blob {
    id: Id,
    pub kind: BlobKind,
    pub owner: Id,
    data: BlobData,
}

impl PartialEq for Blob {
    /// We consider two blobs to be equal if they have the same id.
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

/// The type of the blob.
/// We can later add more types of blobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobKind {
    Executable,
    Data,
}

/// Data type stored by the blob.
#[derive(Debug, Clone)]
pub enum BlobData {
    /// An ELF code that can be executed by the RISC-V processor.
    ELF(Vec<u8>),
    /// A JSON data.
    JSON(Vec<u8>),
}

#[cfg(test)]
impl PartialEq for BlobData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (BlobData::ELF(data1), BlobData::ELF(data2)) => data1 == data2,
            (BlobData::JSON(data1), BlobData::JSON(data2)) => data1 == data2,
            _ => false,
        }
    }
}

impl BlobData {
    /// Create a new BlobData from a given data.
    /// Performs the sanity checks, such as format, size, etc.
    /// Additionally, parses the data and stores it in a more convenient format.
    fn new(kind: BlobKind, data: Vec<u8>) -> Self {
        // TODO - include preprocessing of the data and sanity checks
        match kind {
            BlobKind::Executable => BlobData::ELF(data),
            BlobKind::Data => BlobData::JSON(data),
        }
    }
}

impl Blob {
    /// Generates a unique blob id by hashing the following information:
    /// - `[BlobKind]`
    /// - `owner` of the blob
    /// - provided additional `parameters`, could be a subset of the blob data,
    ///   random bytes, etc.
    fn generate_blob_id<T: Into<Box<[u8]>> + Clone>(
        kind: BlobKind,
        owner: Id,
        parameters: Vec<T>,
    ) -> Id {
        let mut hasher = sha3::Sha3_256::new();
        hasher.update(&[kind as u8]);
        hasher.update(*owner);
        parameters
            .iter()
            .for_each(|p| hasher.update(p.clone().into()));
        let hash = hasher.finalize();

        Id(hash.into())
    }

    /// Create a new Blob from a given data.
    pub fn new<T: Into<Box<[u8]>> + Clone>(
        kind: BlobKind,
        owner: Id,
        id_parameters: Vec<T>,
        data: Vec<u8>,
    ) -> Self {
        Blob {
            id: Self::generate_blob_id(kind, owner, id_parameters),
            kind,
            owner,
            data: match kind {
                BlobKind::Executable => BlobData::ELF(data),
                BlobKind::Data => BlobData::JSON(data),
            },
        }
    }

    /// Get the id of the blob.
    pub fn id(&self) -> &Id { &self.id }

    /// Get the data of the blob.
    pub fn data(&self) -> &BlobData { &self.data }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_blob_id() {
        let blob = Blob::new(
            BlobKind::Data,
            Id::default(),
            vec![vec![1u8]],
            [0u8; 1024].to_vec(),
        );

        assert_eq!(*blob.id, [
            122, 252, 148, 230, 85, 159, 56, 102, 46, 36, 195, 244, 223, 191, 53, 179, 41, 187,
            137, 67, 68, 217, 148, 6, 150, 26, 79, 169, 120, 133, 106, 102
        ])
    }

    #[test]
    fn test_two_blobs_are_equal_by_id() {
        let blob1 = Blob::new(
            BlobKind::Data,
            Id::default(),
            vec![vec![1u8]],
            [0u8; 1024].to_vec(),
        );
        let blob2 = Blob::new(
            BlobKind::Data,
            Id::default(),
            vec![vec![1u8]],
            [0u8; 1024].to_vec(),
        );

        assert_eq!(blob1, blob2);
    }
}
