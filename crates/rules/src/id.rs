//! Identifiers, as opaque values.
//!
//! Adapters parse and validate the wire forms (with `atproto-record`) and hand
//! this crate already-valid values; the rules only compare them. Keeping them
//! opaque here is what lets the rules stay free of SDK types.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! opaque_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

opaque_id!(
    /// A DID: a person's account, or a World's group account.
    Did
);
opaque_id!(
    /// An AT-URI naming a record: `at://did/collection/rkey`.
    RecordUri
);
opaque_id!(
    /// A record version's content identifier.
    Cid
);
opaque_id!(
    /// A scene, identified by its record key in the World's repo or Space.
    SceneId
);
opaque_id!(
    /// Names one authoritative mutation. Retrying with the same ID returns the
    /// same outcome; an ID is never reused for a different request.
    OperationId
);

/// A pinned version: URI plus CID. Replies, admissions, decisions, roll
/// results and reports carry one, so they name exactly what they answered.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StrongRef {
    pub uri: RecordUri,
    pub cid: Cid,
}

impl StrongRef {
    #[must_use]
    pub fn new(uri: RecordUri, cid: Cid) -> Self {
        Self { uri, cid }
    }
}
