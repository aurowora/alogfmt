# alogfmt

Implements an encoder for [logfmt](https://brandur.org/logfmt) using serde.

To use, add the following to `[dependencies]` table in your `Cargo.toml`.

```toml
alogfmt = "^0.1.0"
```

## Usage

The primary interface of this package consists of the `to_string`, `to_bytes`, and `to_writer` functions.
These functions can be used to serialize logfmt-encoded structures to a `String`, `Vec<u8>`, and an `io::Write`
respectively.

```rust
use alogfmt::{to_string, to_bytes, to_writer};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
struct MyStruct {
    pub ts: u64,
    pub message: String,
}

fn main() -> Result<()> {
    let s = MyStruct{
        ts: 1690232215,
        message: String::from("Hello World!"),
    };

    let lf = to_string(&s)?;

    assert_eq!(
        lf,
        r#"ts=1690232215 message="Hello World!""#
    );

    Ok(())
}
```

The `Serializer` implementation is also exported. It wraps an `io::Write` and can be used in a similar manner to `to_writer`.
The `Serializer` is not particularly expensive to construct, so calling `to_writer` should be fine normally. If you'd like to
re-use the `Serializer`, you must call `serializer.reset()` or `serializer.next()` to reset the serializer's internal state such
that it is ready for the next document.

```rust
use alogfmt::Serializer;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
struct MyStruct {
    pub ts: u64,
    pub message: String,
}

fn main() -> Result<()> {
    let s = MyStruct{
        ts: 1690232215,
        message: String::from("Hello World!"),
    };

    let mut serializer = Serializer::new(Vec::new());

    for i in 0..3 {
        s.serialize(&mut serializer)?;
        serializer.next()?;
    }

    // take back the io::Write so we can check the results
    let result = unsafe {
        // The serializer should only ever produce valid utf8, so
        // we can use from_utf8_unchecked to avoid the overhead of
        // checking if the vector is UTF-8 encoded, though the safe
        // from_utf8 would work just as well.
        String::from_utf8_unchecked(serializer.writer())
    };

    assert_eq!(
        result,
        "ts=1690232215 message=\"Hello World!\"\nts=1690232215 message=\"Hello World!\"\nts=1690232215 message=\"Hello World!\"\n"
    );

    Ok(())
}
```

## License

```text
    Copyright (C) 2023 Aurora McGinnis

    This Source Code Form is subject to the terms of the Mozilla Public
    License, v. 2.0. If a copy of the MPL was not distributed with this
    file, You can obtain one at http://mozilla.org/MPL/2.0/.
```
