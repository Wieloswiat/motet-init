//! Wire types for Motet's records, generated from `lexicons/`.
//!
//! These are the shapes on the wire and nothing more: adapters convert them
//! into `motet-rules` values at the edge, after validating them against the
//! schemas in [`SCHEMAS`] (generated) (lengths, formats, known values) with
//! `atproto-lexicon`. Every generated type keeps fields it doesn't know in
//! `extra` and writes them back, so records from newer clients pass through
//! untouched.

#[rustfmt::skip] // generated: lexgen-check compares it byte for byte
mod generated;

pub use generated::*;

use serde::{Deserialize, Serialize};

/// A record type: its collection NSID and record-key kind.
pub trait Record {
    const NSID: &'static str;
    const KEY: &'static str;
}

/// A reference to a blob in the author's repo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blob {
    #[serde(rename = "$type")]
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: CidLink,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: u64,
}

/// A CID in DAG-JSON form: `{"$link": "bafy…"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CidLink {
    #[serde(rename = "$link")]
    pub link: String,
}
