//! MessagePack codec
//!
//! All Saikuro wire encoding goes through this module so host, wasm, and MCU
//! targets emit identical bytes. The underlying encoder is `messagepack-serde`,
//! a `no_std` + alloc MessagePack serializer, so these helpers are available on
//! every build target (the previous rmp-serde codec was std-only).
//!
//! Encoding always uses [`RmpCompatible`], which reproduces the reference
//! rmp-serde byte format exactly: integers are minimized to the smallest
//! representation that holds them and floats keep their native width.
//! `messagepack-serde`'s default `LosslessMinimize` config downcasts `f64`
//! values that fit exactly in `f32`, which would silently change the wire
//! format for `Value::Float`; `RmpCompatible` restores rmp-serde's behavior.
//! Tests in the workspace use rmp-serde as a reference implementation

use alloc::vec::Vec;
use core::convert::Infallible;
use messagepack_serde::{
    messagepack_core::{encode::int::EncodeMinimizeInt, io::IoWrite, io::RError, Encode},
    ser::NumEncoder,
};
use serde::{Deserialize, Serialize};

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

/// Encoding error produced by [`to_vec`].
pub type EncodeError = messagepack_serde::ser::Error<Infallible>;

/// Decoding error produced by [`from_slice`].
pub type DecodeError = messagepack_serde::de::Error<RError>;

/// Serialize a value to MessagePack bytes using the rmp-serde-compatible
/// encoding.
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, EncodeError> {
    messagepack_serde::ser::to_vec_with_config(value, RmpCompatible)
}

/// Deserialize a value from MessagePack bytes.
pub fn from_slice<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, DecodeError> {
    messagepack_serde::de::from_slice(bytes)
}
