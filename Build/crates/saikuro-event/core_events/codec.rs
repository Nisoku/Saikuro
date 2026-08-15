/// Encoding error produced by the MessagePack serializer.
pub type EncodeError = messagepack_serde::ser::Error<core::convert::Infallible>;

/// Decoding error produced by the MessagePack deserializer.
pub type DecodeError = messagepack_serde::de::Error<messagepack_serde::messagepack_core::io::RError>;
