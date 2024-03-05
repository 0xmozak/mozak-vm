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
struct Test {
    int: u8,
    string: String,
    option: Option<Vec<i32>>,
}

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
struct TupleTest(Test, Vec<Test>);

type T = (Test, Vec<Test>);

fn main() {
    {
        let value = Test {
            int: 42,
            string: "hello world".to_string(),
            option: Some(vec![1, 2, 3, 4]),
        };

        let t: T = (value.clone(), vec![value.clone(), value.clone()]);
        let bytes = rkyv::to_bytes::<_, 256>(&t).unwrap();

        // You can use the safe API for fast zero-copy deserialization
        let archived: &(ArchivedTest, ArchivedVec<ArchivedTest>) =
            rkyv::check_archived_root::<T>(&bytes[..]).unwrap();
        assert_eq!(&archived.0, &t.0);
        assert_eq!(&archived.1, &t.1);

        // And you can always deserialize back to the original type
        let deserialized: T = archived.deserialize(&mut rkyv::Infallible).unwrap();
        dbg!(&deserialized);
        assert_eq!(deserialized, t);
    }
    {
        let value = Test {
            int: 42,
            string: "hello world".to_string(),
            option: Some(vec![1, 2, 3, 4]),
        };

        let t = TupleTest(value.clone(), vec![value.clone(), value.clone()]);
        let bytes = rkyv::to_bytes::<_, 256>(&t).unwrap();

        // You can use the safe API for fast zero-copy deserialization
        let archived = rkyv::check_archived_root::<TupleTest>(&bytes[..]).unwrap();
        assert_eq!(archived, &t);

        // And you can always deserialize back to the original type
        let deserialized: TupleTest = archived.deserialize(&mut rkyv::Infallible).unwrap();
        dbg!(&deserialized);
        assert_eq!(deserialized, t);
    }
}
