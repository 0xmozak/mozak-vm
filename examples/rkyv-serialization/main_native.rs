mod core_logic;

use rkyv::Deserialize;

use crate::core_logic::Test;

fn main() {
    let value = Test {
        int: 42,
        string: "Mozak Rocks!!".to_string(),
        option: Some(vec![1, 2, 3, 4]),
    };

    // Serializing is as easy as a single function call
    let bytes = rkyv::to_bytes::<_, 256>(&value).unwrap();

    // Or you can use the unsafe API for maximum performance
    let archived = unsafe { rkyv::archived_root::<Test>(&bytes[..]) };
    assert_eq!(archived, &value);

    // And you can always deserialize back to the original type
    let deserialized: Test = archived.deserialize(&mut rkyv::Infallible).unwrap();
    assert_eq!(deserialized, value);
    println!("Deserialized Value: {:?}", deserialized);
}
