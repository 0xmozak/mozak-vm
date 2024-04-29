#[cfg(target_os = "mozakvm")]
use crate::mozakvm::helpers::poseidon2_hash_no_pad;
#[cfg(not(target_os = "mozakvm"))]
use crate::native::helpers::poseidon2_hash_no_pad;
#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
#[repr(u8)]
pub enum EventType {
    Write = 0,
    Ensure,
    Read,
    Create,
    Delete,
}

impl Default for EventType {
    fn default() -> Self { Self::Read }
}

#[derive(
    Default, Clone, Hash, PartialEq, PartialOrd, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct Event {
    pub object: super::StateObject,
    pub type_: EventType,
}

#[derive(
    Default,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
#[allow(clippy::module_name_repetitions)]
pub struct CanonicalEvent {
    pub address: super::StateAddress,
    pub type_: EventType,
    pub value: super::Poseidon2Hash,
}

impl CanonicalEvent {
    #[must_use]
    pub fn from_event(value: &Event) -> Self {
        #[cfg(not(target_os = "mozakvm"))]
        {
            Self {
                address: value.object.address,
                type_: value.type_,
                value: crate::native::helpers::poseidon2_hash_with_pad(&value.object.data),
            }
        }
        #[cfg(target_os = "mozakvm")]
        {
            Self {
                address: value.object.address,
                type_: value.type_,
                value: crate::mozakvm::helpers::poseidon2_hash_with_pad(&value.object.data),
            }
        }
    }

    #[must_use]
    pub fn canonical_hash(&self) -> super::poseidon2hash::Poseidon2Hash {
        let data_to_hash: Vec<u8> = itertools::chain!(
            u64::from(self.type_ as u8).to_le_bytes(),
            self.address.inner(),
            self.value.inner(),
        )
        .collect();
        poseidon2_hash_no_pad(&data_to_hash)
    }
}

#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[cfg_attr(
    not(target_os = "mozakvm"),
    derive(Debug, serde::Serialize, serde::Deserialize)
)]
pub struct CanonicalOrderedTemporalHints(pub CanonicalEvent, pub u32);

#[cfg(test)]
mod tests {
    // use crate::common::types::{
    //     CanonicalEvent, EventType, Poseidon2Hash, ProgramIdentifier,
    // StateAddress, };
    // use crate::native::helpers::poseidon2_hash_no_pad;

    // #[test]
    // fn check_sample_events_hash() {
    //     // uses test vectors from tests in
    //     // mozak-vm/circuits/src/recproof/verify_event.rs
    //     let program_hash_1 = ProgramIdentifier([4, 8, 15, 16].into());

    //     let zero_val: Poseidon2Hash = [0; 4].into();
    //     let non_zero_val_1: Poseidon2Hash = [3, 1, 4, 15].into();
    //     let non_zero_val_2: Poseidon2Hash = [1, 6, 180, 33].into();
    //     let read_0 = CanonicalEvent {
    //         address: StateAddress(42u64.to_le_bytes()),
    //         emitter: program_hash_1,
    //         type_: EventType::Read,
    //         value: zero_val,
    //     };
    //     let write_1 = CanonicalEvent {
    //         address: StateAddress(42u64.to_le_bytes()),
    //         emitter: program_hash_1,
    //         type_: EventType::Write,
    //         value: non_zero_val_1,
    //     };
    //     let write_2 = CanonicalEvent {
    //         address: StateAddress(42u64.to_le_bytes()),
    //         emitter: program_hash_1,
    //         type_: EventType::Write,
    //         value: non_zero_val_2,
    //     };
    //     let read_0_hash: Poseidon2Hash = [
    //         7272290939186032751,
    //         8185818005188304227,
    //         17555306369107993266,
    //         17187284268557234321,
    //     ]
    //     .into();

    //     let write_1_hash: Poseidon2Hash = [
    //         11469795294276139037,
    //         799622748573506082,
    //         15272809121316752941,
    //         7142640452443475716,
    //     ]
    //     .into();
    //     let write_2_hash: Poseidon2Hash = [
    //         1484423020241144842,
    //         17207848040428508675,
    //         7995793996020726058,
    //         4658801606188332384,
    //     ]
    //     .into();

    //     let branch_1_hash: Poseidon2Hash = [
    //         16758566829994364981,
    //         15311795646108582705,
    //         12773152691662485878,
    //         2551708493265210224,
    //     ]
    //     .into();
    //     let branch_2_hash: Poseidon2Hash = [
    //         8577138257922146843,
    //         5112874340235798754,
    //         4121828782781403483,
    //         12250937462246573507,
    //     ]
    //     .into();

    //     assert_eq!(read_0_hash, read_0.canonical_hash());
    //     assert_eq!(write_1_hash, write_1.canonical_hash());
    //     assert_eq!(write_2_hash, write_2.canonical_hash());

    //     assert_eq!(
    //         branch_1_hash,
    //         poseidon2_hash_no_pad(
    //             &itertools::chain!(
    //                 write_1.canonical_hash().inner(),
    //                 write_2.canonical_hash().inner()
    //             )
    //             .collect::<Vec<u8>>()
    //         )
    //     );

    //     assert_eq!(
    //         branch_2_hash,
    //         poseidon2_hash_no_pad(
    //             &itertools::chain!(read_0.canonical_hash().inner(),
    // branch_1_hash.inner())                 .collect::<Vec<u8>>()
    //         )
    //     )
    // }
}
