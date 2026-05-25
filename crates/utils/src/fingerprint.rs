// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::{
    client_bail,
    error::{Error, Result},
};
use base64::prelude::*;
use serde::Deserialize;
use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

#[derive(Debug)]
pub struct FingerprinterError {
    msg: String,
}

impl std::fmt::Display for FingerprinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FingerprinterError: {}", self.msg)
    }
}
impl std::error::Error for FingerprinterError {}
impl serde::ser::Error for FingerprinterError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        FingerprinterError {
            msg: format!("{msg}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint(pub [u8; 16]);

impl Fingerprint {
    #[inline(always)]
    pub fn to_base64(self) -> String {
        BASE64_STANDARD.encode(self.0)
    }

    #[inline(always)]
    pub fn from_base64(s: &str) -> Result<Self> {
        let bytes = match s.len() {
            24 => BASE64_STANDARD.decode(s)?,
            _ => client_bail!("Encoded fingerprint length is unexpected: {}", s.len()),
        };
        let bytes: [u8; 16] = bytes.try_into().map_err(|e: Vec<u8>| {
            Error::client(format!(
                "Fingerprint bytes length is unexpected: {}",
                e.len()
            ))
        })?;
        Ok(Fingerprint(bytes))
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#")?;
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl AsRef<[u8]> for Fingerprint {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::hash::Hash for Fingerprint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Fingerprint is already evenly distributed, so we can just use the first few bytes.
        const N: usize = size_of::<usize>();
        state.write(&self.0[..N]);
    }
}

impl Serialize for Fingerprint {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_base64(&s).map_err(serde::de::Error::custom)
    }
}
#[derive(Clone, Default)]
pub struct Fingerprinter {
    hasher: blake3::Hasher,
}

impl Fingerprinter {
    #[inline(always)]
    pub fn into_fingerprint(self) -> Fingerprint {
        let mut output = [0u8; 16];
        self.hasher.finalize_xof().fill(&mut output);
        Fingerprint(output)
    }

    #[inline(always)]
    pub fn with<S: Serialize + ?Sized>(
        self,
        value: &S,
    ) -> std::result::Result<Self, FingerprinterError> {
        let mut fingerprinter = self;
        value.serialize(&mut fingerprinter)?;
        Ok(fingerprinter)
    }

    #[inline(always)]
    pub fn write<S: Serialize + ?Sized>(
        &mut self,
        value: &S,
    ) -> std::result::Result<(), FingerprinterError> {
        value.serialize(self)
    }

    #[inline(always)]
    pub fn write_raw_bytes(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    #[inline(always)]
    fn write_type_tag(&mut self, tag: &str) {
        self.hasher.update(tag.as_bytes());
        self.hasher.update(b";");
    }

    #[inline(always)]
    fn write_end_tag(&mut self) {
        self.hasher.update(b".");
    }

    #[inline(always)]
    fn write_varlen_bytes(&mut self, bytes: &[u8]) {
        self.write_usize(bytes.len());
        self.hasher.update(bytes);
    }

    #[inline(always)]
    fn write_usize(&mut self, value: usize) {
        self.hasher.update(&(value as u32).to_le_bytes());
    }
}

impl Serializer for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> std::result::Result<(), Self::Error> {
        self.write_type_tag(if v { "t" } else { "f" });
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("i1");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("i2");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("i4");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("i8");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("u1");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("u2");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("u4");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("u8");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("f4");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("f8");
        self.hasher.update(&v.to_le_bytes());
        Ok(())
    }

    fn serialize_char(self, v: char) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("c");
        self.write_usize(v as usize);
        Ok(())
    }

    fn serialize_str(self, v: &str) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("s");
        self.write_varlen_bytes(v.as_bytes());
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("b");
        self.write_varlen_bytes(v);
        Ok(())
    }

    fn serialize_none(self) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("");
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("()");
        Ok(())
    }

    fn serialize_unit_struct(self, name: &'static str) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("US");
        self.write_varlen_bytes(name.as_bytes());
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> std::result::Result<(), Self::Error> {
        self.write_type_tag("UV");
        self.write_varlen_bytes(name.as_bytes());
        self.write_varlen_bytes(variant.as_bytes());
        Ok(())
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write_type_tag("NS");
        self.write_varlen_bytes(name.as_bytes());
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write_type_tag("NV");
        self.write_varlen_bytes(name.as_bytes());
        self.write_varlen_bytes(variant.as_bytes());
        value.serialize(self)
    }

    fn serialize_seq(
        self,
        _len: Option<usize>,
    ) -> std::result::Result<Self::SerializeSeq, Self::Error> {
        self.write_type_tag("L");
        Ok(self)
    }

    fn serialize_tuple(
        self,
        _len: usize,
    ) -> std::result::Result<Self::SerializeTuple, Self::Error> {
        self.write_type_tag("T");
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> std::result::Result<Self::SerializeTupleStruct, Self::Error> {
        self.write_type_tag("TS");
        self.write_varlen_bytes(name.as_bytes());
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> std::result::Result<Self::SerializeTupleVariant, Self::Error> {
        self.write_type_tag("TV");
        self.write_varlen_bytes(name.as_bytes());
        self.write_varlen_bytes(variant.as_bytes());
        Ok(self)
    }

    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> std::result::Result<Self::SerializeMap, Self::Error> {
        self.write_type_tag("M");
        Ok(self)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> std::result::Result<Self::SerializeStruct, Self::Error> {
        self.write_type_tag("S");
        self.write_varlen_bytes(name.as_bytes());
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> std::result::Result<Self::SerializeStructVariant, Self::Error> {
        self.write_type_tag("SV");
        self.write_varlen_bytes(name.as_bytes());
        self.write_varlen_bytes(variant.as_bytes());
        Ok(self)
    }
}

impl SerializeSeq for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_element<T>(&mut self, value: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

impl SerializeTuple for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_element<T>(&mut self, value: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

impl SerializeTupleStruct for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_field<T>(&mut self, value: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

impl SerializeTupleVariant for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_field<T>(&mut self, value: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

impl SerializeMap for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_key<T>(&mut self, key: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

impl SerializeStruct for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.hasher.update(key.as_bytes());
        self.hasher.update(b"\n");
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

impl SerializeStructVariant for &mut Fingerprinter {
    type Ok = ();
    type Error = FingerprinterError;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.hasher.update(key.as_bytes());
        self.hasher.update(b"\n");
        value.serialize(&mut **self)
    }

    fn end(self) -> std::result::Result<(), Self::Error> {
        self.write_end_tag();
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_fingerprint_to_base64() {
        let bytes = [0u8; 16];
        let fp = Fingerprint(bytes);
        assert_eq!(fp.to_base64(), "AAAAAAAAAAAAAAAAAAAAAA==");

        let bytes_ones = [0xFFu8; 16];
        let fp_ones = Fingerprint(bytes_ones);
        assert_eq!(fp_ones.to_base64(), "/////////////////////w==");

        let bytes_mixed = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        let fp_mixed = Fingerprint(bytes_mixed);
        assert_eq!(fp_mixed.to_base64(), "AQIDBAUGBwgJCgsMDQ4PEA==");
    }

    #[test]
    fn test_fingerprint_from_base64_standard() {
        let bytes = [0u8; 16];
        let base64_str = BASE64_STANDARD.encode(bytes);
        assert_eq!(base64_str.len(), 24);
        let fp = Fingerprint::from_base64(&base64_str).unwrap();
        assert_eq!(fp.0, bytes);
    }

    #[test]
    fn test_fingerprint_from_base64_invalid_length() {
        assert!(Fingerprint::from_base64("too_short").is_err());
        assert!(
            Fingerprint::from_base64(
                "this_string_is_way_too_long_and_definitely_not_a_fingerprint"
            )
            .is_err()
        );
    }

    #[test]
    fn test_fingerprint_from_base64_invalid_encoding() {
        let invalid_base64 = "!!!!####$$$$%%%%^^^^&&&&";
        assert_eq!(invalid_base64.len(), 24);
        assert!(Fingerprint::from_base64(invalid_base64).is_err());
    }

    #[test]
    fn test_fingerprint_from_base64_invalid_decoded_length() {
        // A 24-char base64 string that decodes to more or less than 16 bytes.
        // Standard base64 for 16 bytes is 22 chars + 2 padding '='.
        // BASE64_STANDARD.decode("AQIDBAUGBwgJCgsMDQ4PEBES") would be 18 bytes (24 chars, no padding)
        let invalid_bytes_len_base64 = "AQIDBAUGBwgJCgsMDQ4PEBES";
        assert_eq!(invalid_bytes_len_base64.len(), 24);
        assert!(Fingerprint::from_base64(invalid_bytes_len_base64).is_err());
    }

    #[test]
    fn test_fingerprint_roundtrip() {
        let bytes = [0xABu8; 16];
        let fp = Fingerprint(bytes);
        let base64 = fp.to_base64();
        let fp2 = Fingerprint::from_base64(&base64).unwrap();
        assert_eq!(fp, fp2);
    }

    #[test]
    fn test_fingerprint_display_debug() {
        let bytes = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10,
        ];
        let fp = Fingerprint(bytes);
        let display = format!("{}", fp);
        let debug = format!("{:?}", fp);
        assert_eq!(display, "#0102030405060708090a0b0c0d0e0f10");
        assert_eq!(debug, "#0102030405060708090a0b0c0d0e0f10");
    }

    #[test]
    fn test_fingerprint_serde() {
        let bytes = [0x42u8; 16];
        let fp = Fingerprint(bytes);
        let serialized = serde_json::to_string(&fp).unwrap();
        let expected = format!("\"{}\"", fp.to_base64());
        assert_eq!(serialized, expected);

        let deserialized: Fingerprint = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, fp);
    }

    #[test]
    fn test_fingerprint_hash() {
        let mut set = HashSet::new();
        let fp1 = Fingerprint([1u8; 16]);
        let fp2 = Fingerprint([2u8; 16]);

        set.insert(fp1);
        assert!(set.contains(&fp1));
        assert!(!set.contains(&fp2));

        set.insert(fp1);
        assert_eq!(set.len(), 1);

        set.insert(fp2);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_fingerprint_as_ref_and_slice() {
        let bytes = [0xFEu8; 16];
        let fp = Fingerprint(bytes);
        assert_eq!(fp.as_ref(), &bytes);
        assert_eq!(fp.as_slice(), &bytes);
    }

    // --- Fingerprinter (serde Serializer) ----------------------------------

    use serde::{Serialize, Serializer};
    use std::collections::BTreeMap;

    fn fp_of<S: Serialize + ?Sized>(value: &S) -> Fingerprint {
        Fingerprinter::default()
            .with(value)
            .unwrap()
            .into_fingerprint()
    }

    #[test]
    fn fingerprinter_is_deterministic() {
        assert_eq!(fp_of("hello"), fp_of("hello"));
        assert_eq!(fp_of(&vec![1u32, 2, 3]), fp_of(&vec![1u32, 2, 3]));
    }

    #[test]
    fn fingerprinter_distinguishes_values() {
        assert_ne!(fp_of("hello"), fp_of("world"));
        assert_ne!(fp_of(&1i32), fp_of(&2i32));
        assert_ne!(fp_of(&vec![1u8, 2]), fp_of(&vec![2u8, 1]));
    }

    #[test]
    fn fingerprinter_distinguishes_types_via_tags() {
        // Same numeric value, different width tags -> different fingerprints.
        assert_ne!(fp_of(&1u32), fp_of(&1u64));
        assert_ne!(fp_of(&1i8), fp_of(&1u8));
        assert_ne!(fp_of(&1.0f32), fp_of(&1.0f64));
    }

    #[test]
    fn fingerprinter_handles_scalar_types() {
        // Exercises bool / char / signed / unsigned / float serializer arms.
        let _ = fp_of(&true);
        let _ = fp_of(&false);
        let _ = fp_of(&'z');
        let _ = fp_of(&i16::MIN);
        let _ = fp_of(&i64::MAX);
        let _ = fp_of(&u16::MAX);
        let _ = fp_of(&std::f64::consts::PI);
    }

    #[test]
    fn fingerprinter_option_none_differs_from_some() {
        let none: Option<u32> = None;
        let some: Option<u32> = Some(0);
        assert_ne!(fp_of(&none), fp_of(&some));
    }

    #[test]
    fn fingerprinter_unit_and_unit_struct() {
        #[derive(Serialize)]
        struct UnitStruct;

        // Both serialize via distinct tags; assert they round-trip and differ.
        assert_eq!(fp_of(&()), fp_of(&()));
        assert_ne!(fp_of(&()), fp_of(&UnitStruct));
    }

    #[test]
    fn fingerprinter_map_and_seq() {
        let mut map = BTreeMap::new();
        map.insert("a", 1u32);
        map.insert("b", 2u32);
        assert_eq!(fp_of(&map), fp_of(&map));

        let mut other = BTreeMap::new();
        other.insert("a", 1u32);
        other.insert("b", 3u32);
        assert_ne!(fp_of(&map), fp_of(&other));
    }

    #[test]
    fn fingerprinter_tuple_and_tuple_struct() {
        #[derive(Serialize)]
        struct Pair(u32, &'static str);

        assert_eq!(fp_of(&(1u32, "x")), fp_of(&(1u32, "x")));
        assert_ne!(fp_of(&(1u32, "x")), fp_of(&(1u32, "y")));
        assert_eq!(fp_of(&Pair(1, "x")), fp_of(&Pair(1, "x")));
        assert_ne!(fp_of(&Pair(1, "x")), fp_of(&Pair(2, "x")));
    }

    #[test]
    fn fingerprinter_struct_and_field_order() {
        #[derive(Serialize)]
        struct Record {
            id: u32,
            name: &'static str,
            tags: Vec<&'static str>,
        }

        let a = Record {
            id: 1,
            name: "alpha",
            tags: vec!["x", "y"],
        };
        let b = Record {
            id: 1,
            name: "alpha",
            tags: vec!["x", "y"],
        };
        let c = Record {
            id: 1,
            name: "beta",
            tags: vec!["x", "y"],
        };
        assert_eq!(fp_of(&a), fp_of(&b));
        assert_ne!(fp_of(&a), fp_of(&c));
    }

    #[test]
    fn fingerprinter_enum_variants() {
        #[derive(Serialize)]
        enum Shape {
            Unit,
            Newtype(u32),
            Tuple(u32, u32),
            Struct { w: u32, h: u32 },
        }

        // Each variant kind drives a different serializer arm; all must differ.
        let unit = fp_of(&Shape::Unit);
        let newtype = fp_of(&Shape::Newtype(1));
        let tuple = fp_of(&Shape::Tuple(1, 2));
        let strukt = fp_of(&Shape::Struct { w: 1, h: 2 });

        assert_ne!(unit, newtype);
        assert_ne!(newtype, tuple);
        assert_ne!(tuple, strukt);
        assert_eq!(strukt, fp_of(&Shape::Struct { w: 1, h: 2 }));
        assert_ne!(strukt, fp_of(&Shape::Struct { w: 2, h: 1 }));
    }

    #[test]
    fn fingerprinter_serialize_bytes_arm() {
        // `serialize_bytes` is not reachable via `&[u8]` (serde treats it as a
        // seq), so drive the Serializer method directly.
        let mut a = Fingerprinter::default();
        (&mut a).serialize_bytes(b"abc").unwrap();
        let mut b = Fingerprinter::default();
        (&mut b).serialize_bytes(b"abc").unwrap();
        let mut c = Fingerprinter::default();
        (&mut c).serialize_bytes(b"abc").unwrap();

        assert_eq!(a.into_fingerprint(), b.into_fingerprint());
        assert_ne!(
            Fingerprinter::default()
                .with(b"abc".as_slice())
                .unwrap()
                .into_fingerprint(),
            c.into_fingerprint()
        );
    }

    #[test]
    fn fingerprinter_write_and_raw_bytes() {
        // `write` mutates in place; `write_raw_bytes` injects opaque bytes.
        let mut fp = Fingerprinter::default();
        fp.write(&"key").unwrap();
        fp.write_raw_bytes(b"\x00\x01\x02");
        let one = fp.into_fingerprint();

        let mut fp2 = Fingerprinter::default();
        fp2.write(&"key").unwrap();
        fp2.write_raw_bytes(b"\x00\x01\x02");
        assert_eq!(one, fp2.into_fingerprint());

        let mut fp3 = Fingerprinter::default();
        fp3.write(&"key").unwrap();
        fp3.write_raw_bytes(b"\x00\x01\x03");
        assert_ne!(one, fp3.into_fingerprint());
    }

    #[test]
    fn fingerprinter_error_display_and_custom() {
        use serde::ser::Error as _;
        let err = FingerprinterError::custom("bad value");
        assert_eq!(err.to_string(), "FingerprinterError: bad value");
    }
}
