/*
    Copyright (C) 2023 Aurora McGinnis

    This Source Code Form is subject to the terms of the Mozilla Public
    License, v. 2.0. If a copy of the MPL was not distributed with this
    file, You can obtain one at http://mozilla.org/MPL/2.0/.

    encode.rs: Logfmt serializer implementation.
*/

use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use std::io::Write;

use crate::error::{Error, Result};
use crate::util::as_control_picture;

/// Provides a serde Serializer implementation that is roughly compatible with
/// <https://pkg.go.dev/github.com/kr/logfmt>
///
/// The `Serializer` is not particularly expensive to construct, so calling `to_writer`
/// should be fine normally. If you'd like to re-use the `Serializer`, you must call
/// `serializer.reset()` or `serializer.next()` to reset the serializer's internal state such
/// that it is ready for the next document.
///
/// ```rust
/// use alogfmt::Serializer;
/// use anyhow::Result;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct MyStruct {
///    pub ts: u64,
///    pub message: String,
/// }

/// fn main() -> Result<()> {
///    let s = MyStruct{
///        ts: 1690232215,
///        message: String::from("Hello World!"),
///    };
///
///    let mut serializer = Serializer::new(Vec::new());
///
///    for i in 0..3 {
///        s.serialize(&mut serializer)?;
///        serializer.next()?;
///    }
///
///    // take back the io::Write so we can check the results
///    let result = unsafe {
///        String::from_utf8_unchecked(serializer.writer())
///    };
///
///    assert_eq!(
///        result,
///        "ts=1690232215 message=\"Hello World!\"\nts=1690232215 message=\"Hello World!\"\nts=1690232215 message=\"Hello World!\"\n"
///    );
///
///    Ok(())
/// }
/// ```
pub struct Serializer<B> {
    w: B,
    ns: Vec<String>,
    have_written: bool,
}

impl<B> Serializer<B>
where
    B: Write,
{
    /// Construct a new `LogfmtSerializer` that writes to
    /// the supplied object implementing `Write`.
    pub fn new(writer: B) -> Self {
        Serializer {
            w: writer,
            ns: Vec::with_capacity(8),
            have_written: false,
        }
    }

    /// Reclaim the writer wrapped by this serializer.
    pub fn writer(self) -> B {
        self.w
    }

    /// Prepare the serializer for the next document by calling `self.reset()` and
    /// writing a new line character to the output.
    ///
    /// # Errors
    /// This function will fail if the underlying IO object
    /// returns an error while writing to it.
    #[inline]
    pub fn next(&mut self) -> Result<()> {
        self.w.write_all(b"\n")?;
        self.reset();
        Ok(())
    }

    /// Resets the serializer's internal state
    /// such that it can be used again for the next
    /// object.
    #[inline]
    pub fn reset(&mut self) {
        self.ns.clear();
        self.have_written = false;
    }

    fn enter_ns<S>(&mut self, name: &S)
    where
        S: ToString,
    {
        self.ns.push(name.to_string());
    }

    #[inline]
    fn leave_ns(&mut self) -> String {
        if let Some(ns) = self.ns.pop() {
            ns
        } else {
            panic!("leave_ns() called while in the top level name space");
        }
    }

    // Returns true if the character is valid in logfmt identifiers
    #[inline]
    fn valid_in_ident(c: char) -> bool {
        c > ' ' && c != '=' && c != '"' && !c.is_control()
    }

    // Writes an identifier to the underlying stream.
    // If the identifier has zero-length, then this
    // function returns an error. Invalid bytes are
    // escaped.
    fn write_ident(w: &mut B, ident: &str) -> Result<()> {
        if ident.is_empty() {
            return Err(Error::EmptyIdentifier);
        }

        let (mut beg, mut end): (usize, usize) = (0, 0);

        for ch in ident.chars() {
            if Self::valid_in_ident(ch) {
                end += ch.len_utf8();
            } else {
                if end - beg > 0 {
                    w.write_all(ident[beg..end].as_bytes())?;
                }

                let mut buf: [u8; 4] = [0; 4];
                for b in ch.encode_utf8(&mut buf).as_bytes() {
                    w.write_all(b"%")?;
                    w.write_all(&base16::encode_byte_u(*b))?;
                }
                end += ch.len_utf8();
                beg = end;
            }
        }

        if end - beg > 0 {
            w.write_all(ident[beg..end].as_bytes())?;
        }

        Ok(())
    }

    #[inline]
    fn valid_in_string(c: char) -> bool {
        c >= ' ' && c != '"' && c != '\\' && c != '\x7F'
    }

    #[inline]
    fn is_valid_escape(c: char) -> bool {
        c == 'n'
            || c == '0'
            || c == 't'
            || c == 'r'
            || c == '\\'
            || c == '"'
            || c == 'x' // won't validate is valid ascii
            || c == 'u' // won't validate is valid unicode
    }

    #[inline]
    fn write_escape(dst: &mut B, c: char) -> Result<()> {
        let mut buf: [u8; 4] = [0; 4];

        let b = match c {
            '\0' => b"\\0",
            '\n' => b"\\n",
            '\t' => b"\\t",
            '\r' => b"\\r",
            '\0'..='\x1F' | '\x7F' => as_control_picture(c)
                .expect("function handles ascii codes [0x0, 0x1F] and 0x7F")
                .encode_utf8(&mut buf)
                .as_bytes(),
            '\\' => b"\\\\",
            '"' => b"\\\"",
            _ => c.encode_utf8(&mut buf).as_bytes(),
        };

        dst.write_all(b)?;

        Ok(())
    }

    // Writes a logfmt value to the underlying stream.
    // The value has one of four representations
    // 1) If the value is a valid identifier, the value is represented without quotes
    // 2) If the value is not a valid identifier or requires escapes, it is quoted
    // and is escaped as necessary
    // 3) If the value has zero length, nothing is written.
    fn write_val(&mut self, val: &str) -> Result<()> {
        if val.is_empty() {
            return Ok(());
        }

        // if it's a valid ident, we can just write it as one
        let is_ident = {
            let mut ok = true;

            for ch in val.chars() {
                if !Self::valid_in_ident(ch) {
                    ok = false;
                    break;
                }
            }

            ok
        };
        if is_ident {
            return Self::write_ident(&mut self.w, val);
        }

        // needs quotes
        self.w.write_all(b"\"")?;

        let (mut beg, mut end): (usize, usize) = (0, 0);

        let mut iter = val.chars().peekable();
        while let Some(ch) = iter.next() {
            if Self::valid_in_string(ch) {
                end += ch.len_utf8();
            } else if ch == '\\'
                && iter.peek().is_some()
                && Self::is_valid_escape(*iter.peek().unwrap())
            {
                end += ch.len_utf8() + iter.peek().unwrap().len_utf8();
                // consume the next one too
                let _ = iter.next();
            } else {
                if end - beg > 0 {
                    self.w.write_all(val[beg..end].as_bytes())?;
                }

                Self::write_escape(&mut self.w, ch)?;
                end += ch.len_utf8();
                beg = end;
            }
        }

        if end - beg > 0 {
            self.w.write_all(val[beg..end].as_bytes())?;
        }

        self.w.write_all(b"\"")?;

        Ok(())
    }

    // Returns true if a key was written
    fn write_key(&mut self) -> Result<bool> {
        if self.have_written {
            self.w.write_all(b" ")?;
        } else {
            self.have_written = true;
        }

        if self.ns.is_empty() {
            return Ok(false);
        }

        for (idx, ns) in self.ns.iter().enumerate() {
            Self::write_ident(&mut self.w, ns)?;

            if idx + 1 < self.ns.len() {
                self.w.write_all(b".")?;
            }
        }

        Ok(true)
    }
}

impl<'a, B> ser::Serializer for &'a mut Serializer<B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = LogfmtSeqSerializer<'a, B>;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeTuple = LogfmtSeqSerializer<'a, B>;
    type SerializeTupleStruct = LogfmtSeqSerializer<'a, B>;
    type SerializeTupleVariant = LogfmtSeqSerializer<'a, B>;
    type SerializeStructVariant = Self;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        if v {
            self.write_key()?;
        }

        Ok(())
    }

    #[inline]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = dtoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = dtoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        let mut buf = itoa::Buffer::new();
        self.w.write_all(buf.format(v).as_bytes())?;
        Ok(())
    }

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    #[inline]
    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        self.write_val(v)
    }

    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        let encoded = base16::encode_upper(v);
        self.serialize_str(&encoded)
    }

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        self.w.write_all(b"null")?;
        Ok(())
    }

    #[inline]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    // Treat the same as an empty string (i.e. nothing)
    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok> {
        if self.write_key()? {
            self.w.write_all(b"=")?;
        }

        Ok(())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        self.serialize_unit()
    }

    // Write the name of the enum::variant

    #[inline]
    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        let mut s = String::with_capacity(name.len() + variant.len() + 2);
        s.push_str(name);
        s.push_str("::");
        s.push_str(variant);
        self.serialize_str(&s)?;

        Ok(())
    }

    #[inline]
    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(LogfmtSeqSerializer { s: self, idx: 0 })
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(LogfmtSeqSerializer { s: self, idx: 0 })
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(LogfmtSeqSerializer { s: self, idx: 0 })
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Ok(LogfmtSeqSerializer { s: self, idx: 0 })
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(self)
    }

    #[inline]
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(self)
    }
}

impl<'a, B> SerializeStruct for &'a mut Serializer<B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.enter_ns(&key);

        if let Err(e) = value.serialize(&mut **self) {
            self.leave_ns();
            return Err(e);
        }

        self.leave_ns();
        Ok(())
    }

    #[inline]
    fn skip_field(&mut self, _key: &'static str) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, B> SerializeStructVariant for &'a mut Serializer<B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.enter_ns(&key);

        if let Err(e) = value.serialize(&mut **self) {
            self.leave_ns();
            return Err(e);
        }

        self.leave_ns();
        Ok(())
    }

    #[inline]
    fn skip_field(&mut self, _key: &'static str) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, B> SerializeMap for &'a mut Serializer<B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let mut key_as_logfmt = Serializer {
            w: Vec::with_capacity(64),
            ns: Vec::with_capacity(8),
            have_written: false,
        };

        key.serialize(&mut key_as_logfmt)?;
        let k = unsafe { String::from_utf8_unchecked(key_as_logfmt.w) };
        self.enter_ns(&k);

        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        if let Err(e) = value.serialize(&mut **self) {
            self.leave_ns();
            return Err(e);
        }
        self.leave_ns();
        Ok(())
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: serde::Serialize,
        V: serde::Serialize,
    {
        self.serialize_key(key)?;
        self.serialize_value(value)?;
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

#[doc(hidden)]
/// Type to help serialize sequences.
pub struct LogfmtSeqSerializer<'a, B> {
    s: &'a mut Serializer<B>,
    idx: usize,
}

impl<'a, B> LogfmtSeqSerializer<'a, B>
where
    B: Write,
{
    #[inline]
    fn serialize_element_internal<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let mut buf = itoa::Buffer::new();
        self.s.enter_ns(&buf.format(self.idx));

        if let Err(e) = value.serialize(&mut *self.s) {
            self.s.leave_ns();
            return Err(e);
        }

        self.s.leave_ns();

        self.idx += 1;

        Ok(())
    }
}

impl<'a, B> SerializeSeq for LogfmtSeqSerializer<'a, B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.serialize_element_internal(value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, B> SerializeTuple for LogfmtSeqSerializer<'a, B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.serialize_element_internal(value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, B> SerializeTupleStruct for LogfmtSeqSerializer<'a, B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.serialize_element_internal(value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, B> SerializeTupleVariant for LogfmtSeqSerializer<'a, B>
where
    B: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.serialize_element_internal(value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Result, Serializer};

    #[test]
    fn test_write_ident() {
        fn try_ident(s: &str) -> Result<String> {
            let mut v = Vec::new();

            Serializer::write_ident(&mut v, s)?;

            Ok(unsafe { String::from_utf8_unchecked(v) })
        }

        assert_eq!(try_ident("hello").unwrap(), "hello");
        assert_eq!(try_ident("has spaces").unwrap(), "has%20spaces");
        assert_eq!(try_ident("with=equals").unwrap(), "with%3Dequals");
        assert_eq!(try_ident("with\"quotes").unwrap(), "with%22quotes");
        assert_eq!(try_ident("spaceattheend ").unwrap(), "spaceattheend%20");
        assert_eq!(try_ident("=equalsbeg").unwrap(), "%3Dequalsbeg");
        assert_eq!(try_ident("!\0").unwrap(), "!%00");
        assert!(try_ident("").is_err())
    }

    #[test]
    fn test_write_val() {
        fn try_val(s: &str) -> Result<String> {
            let mut ser = Serializer::new(Vec::new());

            ser.write_val(s)?;

            Ok(unsafe { String::from_utf8_unchecked(ser.w) })
        }

        assert_eq!(try_val("").unwrap(), "");
        assert_eq!(try_val("ident").unwrap(), "ident");
        assert_eq!(try_val("\"").unwrap(), "\"\\\"\"");
        assert_eq!(
            try_val("this one has a space").unwrap(),
            "\"this one has a space\""
        );
        assert_eq!(
            try_val("this \\n \\0 \\t \\r \\\\ \\\" is already escaped").unwrap(),
            "\"this \\n \\0 \\t \\r \\\\ \\\" is already escaped\""
        );
        assert_eq!(
            try_val("needs escaped \n").unwrap(),
            "\"needs escaped \\n\""
        );
    }
}
