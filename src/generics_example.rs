use core::fmt::Debug;
use rkyv::vec::ArchivedVec;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Clone, Deserialize, Serialize, Debug, PartialEq)]
#[archive(
    // This will generate a PartialEq impl between our unarchived and archived
    // types:
    compare(PartialEq),
    // bytecheck can be used to validate your data if you want. To use the safe
    // API, you have to derive CheckBytes for the archived type:
    check_bytes,
)]
// Derives can be passed through to the generated type:
#[archive_attr(derive(Debug))]
struct T2<S: Archive, T: Archive>(S, Vec<T>)
where
    S::Archived: Debug,
    T::Archived: Debug,
    <Vec<T> as Archive>::Archived: Debug;

pub fn main() {
    println!("Generics Example");
    type T2_ = T2<u8, u32>;
    let t: T2_ = T2(42, vec![1, 2, 3, 4]);
    let bytes = rkyv::to_bytes::<_, 256>(&t).unwrap();

    // You can use the safe API for fast zero-copy deserialization
    let archived: &_ = rkyv::check_archived_root::<T2_>(&bytes[..]).unwrap();
    assert_eq!(&archived.0, &t.0);
    assert_eq!(&archived.1, &t.1);

    // And you can always deserialize back to the original type
    let deserialized: T2_ = archived.deserialize(&mut rkyv::Infallible).unwrap();
    dbg!(&deserialized);
    assert_eq!(deserialized, t);
}
