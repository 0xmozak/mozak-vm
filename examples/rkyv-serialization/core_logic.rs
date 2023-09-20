use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive(
    // This will generate a PartialEq impl between our unarchived and archived
    // types:
    compare(PartialEq),
)]
// Derives can be passed through to the generated type:
#[archive_attr(derive(Debug))]
pub struct Test {
    pub int: u8,
    pub string: String,
    pub option: Option<Vec<i32>>,
}
