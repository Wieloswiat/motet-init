//! Motet's lexicon code generator.
//!
//! Lexicons are the contract; the Rust in `crates/lexicons` and the
//! TypeScript in `packages/lexicons` are generated from them and checked in,
//! so reviews show what a schema change does to the code. `check` mode fails
//! CI when the two drift.
//!
//! Deliberately small: it covers what Motet's lexicons use (records, objects,
//! strings, integers, booleans, blobs, CID links, unknown, refs and arrays).
//! Queries, procedures, subscriptions and unions come with the first XRPC
//! methods. Validation against the schema (lengths, formats, known values)
//! stays with `atproto-lexicon` at ingest; generated types only give shape.
//!
//! Output is deterministic: definitions, fields and files are sorted, so the
//! same lexicons always produce byte-identical code.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub mod rust;
pub mod ts;

/// One lexicon document.
#[derive(Clone, Debug, Deserialize)]
pub struct Doc {
    pub lexicon: u32,
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub defs: BTreeMap<String, Def>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Def {
    Record {
        #[serde(default)]
        description: Option<String>,
        key: String,
        record: Object,
    },
    Object(Object),
    #[serde(other)]
    Unsupported,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Object {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub properties: BTreeMap<String, Field>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Field {
    String {
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        format: Option<String>,
    },
    Integer {
        #[serde(default)]
        description: Option<String>,
    },
    Boolean {
        #[serde(default)]
        description: Option<String>,
    },
    Blob {
        #[serde(default)]
        description: Option<String>,
    },
    CidLink {
        #[serde(default)]
        description: Option<String>,
    },
    Unknown {
        #[serde(default)]
        description: Option<String>,
    },
    Ref {
        #[serde(default)]
        description: Option<String>,
        #[serde(rename = "ref")]
        target: String,
    },
    Array {
        #[serde(default)]
        description: Option<String>,
        items: Box<Field>,
    },
}

impl Field {
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::String { description, .. }
            | Self::Integer { description }
            | Self::Boolean { description }
            | Self::Blob { description }
            | Self::CidLink { description }
            | Self::Unknown { description }
            | Self::Ref { description, .. }
            | Self::Array { description, .. } => description.as_deref(),
        }
    }
}

/// A reference resolved to the document and definition it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub doc: String,
    pub def: String,
}

impl Target {
    /// Resolves `#def`, `nsid` or `nsid#def` from inside `doc`.
    pub fn resolve(doc: &str, reference: &str) -> Self {
        match reference.split_once('#') {
            Some(("", def)) => Self {
                doc: doc.to_owned(),
                def: def.to_owned(),
            },
            Some((nsid, def)) => Self {
                doc: nsid.to_owned(),
                def: def.to_owned(),
            },
            None => Self {
                doc: reference.to_owned(),
                def: "main".to_owned(),
            },
        }
    }
}

/// The type name for a definition: `main` takes the NSID's last segment.
pub fn type_name(doc: &str, def: &str) -> String {
    let base = if def == "main" {
        doc.rsplit('.').next().unwrap_or(doc)
    } else {
        def
    };
    let mut out = String::new();
    let mut upper = true;
    for c in base.chars() {
        if c == '-' || c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Loads every `*.json` lexicon under `dir`, keyed by NSID, and checks that
/// every reference points at a definition that exists.
pub fn load(dir: &Path) -> Result<BTreeMap<String, Doc>> {
    let mut docs = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "json") {
                let text = std::fs::read_to_string(&path)?;
                let doc: Doc = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                if doc.lexicon != 1 {
                    bail!("{}: unsupported lexicon version {}", path.display(), doc.lexicon);
                }
                let expected: std::path::PathBuf =
                    doc.id.split('.').collect::<std::path::PathBuf>().with_extension("json");
                if !path.ends_with(&expected) {
                    bail!("{}: id {} belongs at {}", path.display(), doc.id, expected.display());
                }
                docs.insert(doc.id.clone(), doc);
            }
        }
    }
    for doc in docs.values() {
        for (name, def) in &doc.defs {
            if matches!(def, Def::Unsupported) {
                bail!("{}#{name}: definition type not supported by motet-lexgen yet", doc.id);
            }
            let fields = match def {
                Def::Record { record, .. } => &record.properties,
                Def::Object(o) => &o.properties,
                Def::Unsupported => unreachable!(),
            };
            for (field, ty) in fields {
                check_refs(&docs, &doc.id, &format!("{}#{name}.{field}", doc.id), ty)?;
            }
        }
    }
    Ok(docs)
}

fn check_refs(docs: &BTreeMap<String, Doc>, doc: &str, at: &str, field: &Field) -> Result<()> {
    match field {
        Field::Ref { target, .. } => {
            let t = Target::resolve(doc, target);
            let found = docs.get(&t.doc).is_some_and(|d| d.defs.contains_key(&t.def));
            if !found {
                bail!("{at}: reference {target} doesn't resolve (vendor the lexicon under lexicons/?)");
            }
            Ok(())
        }
        Field::Array { items, .. } => check_refs(docs, doc, at, items),
        _ => Ok(()),
    }
}

/// The file stem a document generates into: `at.motet.pilot.post` → `at_motet_pilot_post`.
pub fn module_name(nsid: &str) -> String {
    nsid.replace(['.', '-'], "_").to_lowercase()
}
