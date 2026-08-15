use alloc::vec::Vec;
use messagepack_serde::{
    messagepack_core::{encode::int::EncodeMinimizeInt, io::IoWrite, Encode},
    ser::NumEncoder,
};
use serde::{Deserialize, Serialize};
use saikuro_event::{DecodeError, EncodeError};

/// Encodes numbers exactly like rmp-serde
struct RmpCompatible;

impl<W: IoWrite> NumEncoder<W> for RmpCompatible {
    fn encode_i8(
        v: i8,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_i16(
        v: i16,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_i32(
        v: i32,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_i64(
        v: i64,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_i128(
        v: i128,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_u8(
        v: u8,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_u16(
        v: u16,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_u32(
        v: u32,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_u64(
        v: u64,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_u128(
        v: u128,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        EncodeMinimizeInt(v).encode(writer)
    }

    fn encode_f32(
        v: f32,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        v.encode(writer)
    }

    fn encode_f64(
        v: f64,
        writer: &mut W,
    ) -> Result<usize, messagepack_serde::messagepack_core::encode::Error<W::Error>> {
        v.encode(writer)
    }
}

/// Serialize a value to MessagePack bytes.
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, EncodeError> {
    messagepack_serde::ser::to_vec_with_config(value, RmpCompatible)
}

/// Deserialize a value from MessagePack bytes.
pub fn from_slice<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, DecodeError> {
    messagepack_serde::de::from_slice(bytes)
}
