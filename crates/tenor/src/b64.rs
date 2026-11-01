//! Bytes as base64 strings in the shard's JSON log segments and stored
//! events, instead of serde's default array of numbers, which is about four
//! times the size.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use serde::{Deserialize, Deserializer, Serializer};

pub(crate) fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&STANDARD.encode(bytes))
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Bytes, D::Error> {
    let text = <&str>::deserialize(deserializer)?;
    STANDARD.decode(text).map(Bytes::from).map_err(serde::de::Error::custom)
}

/// The same for optional bytes: `None` is JSON `null`.
pub(crate) mod option {
    use super::{Bytes, Deserialize, Deserializer, STANDARD, Serializer};
    use base64::Engine as _;

    #[expect(
        clippy::ref_option,
        reason = "serde's `with` passes the field by reference, so the signature is fixed"
    )]
    pub(crate) fn serialize<S: Serializer>(bytes: &Option<Bytes>, serializer: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => serializer.serialize_some(&STANDARD.encode(bytes)),
            None => serializer.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Bytes>, D::Error> {
        Option::<&str>::deserialize(deserializer)?
            .map(|text| STANDARD.decode(text).map(Bytes::from).map_err(serde::de::Error::custom))
            .transpose()
    }
}
