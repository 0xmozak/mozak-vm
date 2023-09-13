use std::ops::Deref;

use sha3::digest::FixedOutput;
use sha3::Digest;

use crate::vm::ELF;
use crate::Id;

/// A struct that represents a single data blob on the network. This can either
/// be a program, or just a stored data. This is analogous to a regular File in
/// computer system.
#[derive(Debug)]
pub struct Blob {
    id: Id,
    details: BlobDetails,
    pub owner: Id,
}

/// The type of the blob.
/// We can later add more types of blobs.
#[derive(Debug, Clone)]
pub enum BlobDetails {
    Executable(ELF),
    Data(JSON),
}

/// JSON data.
/// TODO - replace with JSON type
#[derive(Debug, Clone)]
pub struct JSON {
    content: String,
}

impl PartialEq for Blob {
    /// We consider two blobs to be equal if they have the same id.
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

impl Blob {
    /// Generates a unique blob id by hashing the following information:
    /// - `[BlobKind]`
    /// - `owner` of the blob
    /// - provided additional `parameters`, could be a subset of the blob data,
    ///   random bytes, etc.
    fn generate_blob_id<T: Into<Box<[u8]>> + Clone>(
        is_executable: bool,
        owner: Id,
        parameters: Vec<T>,
    ) -> Id {
        let mut hasher = sha3::Sha3_256::new();
        hasher.update(&[is_executable as u8]);
        hasher.update(*owner);
        parameters
            .iter()
            .for_each(|p| hasher.update(p.clone().into()));
        let hash = hasher.finalize();

        Id(hash.into())
    }

    /// Create a new Blob from given data.
    pub fn new<T: Into<Box<[u8]>> + Clone>(
        is_executable: bool,
        owner: Id,
        id_parameters: Vec<T>,
        data: Vec<u8>,
    ) -> Self {
        Blob {
            id: Self::generate_blob_id(is_executable, owner, id_parameters),
            details: Self::parse_blob_data(is_executable, data),
            owner,
        }
    }

    /// Parses the blob data into a specific type.
    fn parse_blob_data(is_executable: bool, data: Vec<u8>) -> BlobDetails {
        if is_executable {
            BlobDetails::Executable(ELF {
                entry_point: 0,
                size: 0,
                code: data,
            })
        } else {
            BlobDetails::Data(JSON {
                content: String::from_utf8(data).unwrap(),
            })
        }
    }

    /// Get the id of the blob.
    pub fn id(&self) -> &Id { &self.id }

    /// Get the blob as a program
    #[inline]
    pub fn as_program(&self) -> Option<&ELF> {
        match self {
            Blob {
                details: BlobDetails::Executable(elf),
                ..
            } => Some(&elf),
            _ => None,
        }
    }

    /// Get the content of the blob
    pub(crate) fn data(&self) -> Vec<u8> {
        match self {
            Blob {
                details: BlobDetails::Data(json),
                ..
            } => json.content.clone().into_bytes(),
            Blob {
                details: BlobDetails::Executable(elf),
                ..
            } => elf.into(),
        }
    }

    #[allow(unused_variables)] // TODO - remove
    pub(crate) fn is_executable(&self) -> bool {
        match self {
            Blob {
                details: BlobDetails::Executable(elf),
                ..
            } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_blob_id() {
        let blob = Blob::new(true, Id::default(), vec![vec![1u8]], [0u8; 1024].to_vec());

        assert_eq!(*blob.id, [
            122, 252, 148, 230, 85, 159, 56, 102, 46, 36, 195, 244, 223, 191, 53, 179, 41, 187,
            137, 67, 68, 217, 148, 6, 150, 26, 79, 169, 120, 133, 106, 102
        ])
    }

    #[test]
    fn test_two_blobs_are_equal_by_id() {
        let blob1 = Blob::new(true, Id::default(), vec![vec![1u8]], [0u8; 1024].to_vec());
        let blob2 = Blob::new(true, Id::default(), vec![vec![1u8]], [1u8; 1024].to_vec());

        assert_eq!(blob1, blob2);
    }
}
