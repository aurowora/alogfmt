/*
    Copyright (C) 2023 Aurora McGinnis

    This Source Code Form is subject to the terms of the Mozilla Public
    License, v. 2.0. If a copy of the MPL was not distributed with this
    file, You can obtain one at http://mozilla.org/MPL/2.0/.

    lib.rs: Export certain types and provide serde to_* functions.
*/
mod encode;
mod error;
mod util;
use std::io::Write;

pub use encode::Serializer;
pub use error::{Error, Result};
use serde::ser::Serialize;

/// Serializes an object to logfmt and returns the result as a string.
///
/// # Errors
/// This function will return an error if the underlying writer encounters an
/// IO error, an attempt is made to write an empty identifier, or the `Serialize`
/// implementation on T fails.
pub fn to_string<T: Serialize>(obj: &T) -> Result<String> {
    // The encoder only produces valid UTF-8
    Ok(unsafe { String::from_utf8_unchecked(to_bytes(obj)?) })
}

/// Serializes an object to logfmt and returns the result as bytes.
///
/// # Errors
/// This function will return an error if the underlying writer encounters an
/// IO error, an attempt is made to write an empty identifier, or the `Serialize`
/// implementation on T fails.
pub fn to_bytes<T: Serialize>(obj: &T) -> Result<Vec<u8>> {
    let mut serializer = Serializer::new(Vec::with_capacity(256));
    obj.serialize(&mut serializer)?;

    Ok(serializer.writer())
}

/// Serializes an object to logfmt and writes the result to the provided writer.
///
/// # Errors
/// This function will return an error if the underlying writer encounters an
/// IO error, an attempt is made to write an empty identifier, or the `Serialize`
/// implementation on T fails.
pub fn to_writer<T: Serialize, W: Write>(dst: &mut W, src: &T) -> Result<()> {
    let mut serializer = Serializer::new(dst);
    src.serialize(&mut serializer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::to_writer;

    use super::to_string;
    use serde::Serialize;
    use std::collections::HashMap;

    // Some types to play with
    #[derive(Serialize)]
    struct MyStruct<'a> {
        message: String,
        integer: i64,
        enum_val: MyEnum,
        #[serde(with = "serde_bytes")]
        b: &'a [u8],
        nums: [i32; 4],
        my_map: HashMap<usize, bool>,
    }

    #[allow(dead_code)]
    #[derive(Serialize)]
    enum MyEnum {
        Variant1,
        Variant2,
        Variant3(u128),
    }

    #[test]
    fn serialize_struct() {
        let mut m = HashMap::new();
        m.insert(33, true);
        m.insert(34, false);

        let mut my_struct = MyStruct {
            message: String::from("hello world"),
            integer: 3829,
            enum_val: MyEnum::Variant2,
            b: &[0xFF, 0x01, 0x43, 0x64],
            nums: [1, 2, 3, 4],
            my_map: m,
        };

        assert_eq!(
            to_string(&my_struct).unwrap(),
            "message=\"hello world\" integer=3829 enum_val=MyEnum::Variant2 b=FF014364 nums.0=1 nums.1=2 nums.2=3 nums.3=4 my_map.33"
        );

        my_struct.message = "\x7FHello World".to_owned();

        assert_eq!(
            to_string(&my_struct).unwrap(),
            "message=\"␡Hello World\" integer=3829 enum_val=MyEnum::Variant2 b=FF014364 nums.0=1 nums.1=2 nums.2=3 nums.3=4 my_map.33"
        );

        my_struct.message = "\n \\n".to_owned();
        my_struct.enum_val = MyEnum::Variant3(389384893);
        assert_eq!(
            to_string(&my_struct).unwrap(),
            "message=\"\\n \\n\" integer=3829 enum_val=389384893 b=FF014364 nums.0=1 nums.1=2 nums.2=3 nums.3=4 my_map.33"
        )
    }

    #[test]
    fn test_to_writer() {
        let mut v = Vec::new();

        let mut m = HashMap::new();
        m.insert(33, true);
        m.insert(34, false);

        let my_struct = MyStruct {
            message: String::from("hello world"),
            integer: 3829,
            enum_val: MyEnum::Variant2,
            b: &[0xFF, 0x01, 0x43, 0x64],
            nums: [1, 2, 3, 4],
            my_map: m,
        };

        to_writer(&mut v, &my_struct).unwrap();

        assert_eq!(
            unsafe { String::from_utf8_unchecked(v) },
            "message=\"hello world\" integer=3829 enum_val=MyEnum::Variant2 b=FF014364 nums.0=1 nums.1=2 nums.2=3 nums.3=4 my_map.33"
        );
    }
}
