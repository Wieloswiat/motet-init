//! Motet's lexicon code generator.
//!
//! Lexicons are the contract; the Rust in `crates/lexicons` and the
//! TypeScript in `packages/lexicons` are generated from them and checked in,
//! so reviews show what a schema change does to the code. `check` mode fails
//! CI when the two drift.
//!
//! Records, objects, RPC parameters and JSON input/output bodies generate wire
//! types. Strings, including `knownValues`, stay open; schema validation belongs
//! to `atproto-lexicon` at ingest. Unions carry `$type` and preserve unknown
//! variants unless the schema closes them. Bytes and CID links use DAG-JSON.
//!
//! Space and permission-set definitions describe host policy, not wire values.
//! Their modules expose identifiers and explain the omission; [`rust::generate`]
//! still embeds their complete schemas. Tokens generate string identifiers.
//! Subscriptions and RPC bodies other than JSON objects or refs are refused.
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
    String {
        #[serde(default)]
        description: Option<String>,
        #[serde(default, rename = "knownValues")]
        known_values: Vec<String>,
    },
    Token {
        #[serde(default)]
        description: Option<String>,
    },
    Space {
        #[serde(default)]
        description: Option<String>,
    },
    PermissionSet {
        #[serde(default)]
        description: Option<String>,
    },
    Query(Rpc),
    Procedure(Rpc),
    #[serde(other)]
    Unsupported,
}

/// The JSON wire shapes of an XRPC method, without its authorization policy.
#[derive(Clone, Debug, Deserialize)]
pub struct Rpc {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<Object>,
    #[serde(default)]
    pub input: Option<Body>,
    #[serde(default)]
    pub output: Option<Body>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Body {
    pub encoding: String,
    pub schema: BodySchema,
}

/// Refuse unsupported schemas rather than silently generating an empty body.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum BodySchema {
    Object(Object),
    Ref {
        #[serde(rename = "ref")]
        target: String,
    },
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
        #[serde(default, rename = "knownValues")]
        known_values: Vec<String>,
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
    Bytes {
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
    Union {
        #[serde(default)]
        description: Option<String>,
        refs: Vec<String>,
        #[serde(default)]
        closed: bool,
    },
    Array {
        #[serde(default)]
        description: Option<String>,
        items: Box<Self>,
    },
}

impl Field {
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::String { description, .. }
            | Self::Integer { description }
            | Self::Boolean { description }
            | Self::Blob { description }
            | Self::CidLink { description }
            | Self::Bytes { description }
            | Self::Unknown { description }
            | Self::Ref { description, .. }
            | Self::Union { description, .. }
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
    #[must_use]
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

    /// The canonical `$type` tag, without a redundant `#main` fragment.
    #[must_use]
    pub fn reference(&self) -> String {
        if self.def == "main" {
            self.doc.clone()
        } else {
            format!("{}#{}", self.doc, self.def)
        }
    }
}

/// The type name for a definition: `main` takes the NSID's last segment.
#[must_use]
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
            let at = format!("{}#{name}", doc.id);
            match def {
                Def::Record { record, .. } | Def::Object(record) => {
                    check_object_refs(&docs, &doc.id, &at, record)?;
                }
                Def::Query(rpc) | Def::Procedure(rpc) => {
                    if let Some(parameters) = &rpc.parameters {
                        check_object_refs(&docs, &doc.id, &format!("{at}.parameters"), parameters)?;
                    }
                    for (part, body) in [("input", &rpc.input), ("output", &rpc.output)] {
                        if let Some(body) = body {
                            if body.encoding != "application/json" {
                                bail!("{at}.{part}: only application/json bodies are supported");
                            }
                            let at = format!("{at}.{part}");
                            match &body.schema {
                                BodySchema::Object(object) => check_object_refs(&docs, &doc.id, &at, object)?,
                                BodySchema::Ref { target } => check_object_ref(&docs, &doc.id, &at, target)?,
                            }
                        }
                    }
                }
                Def::String { .. } | Def::Token { .. } | Def::Space { .. } | Def::PermissionSet { .. } => {}
                Def::Unsupported => bail!("{at}: definition type not supported by motet-lexgen yet"),
            }
        }
    }
    Ok(docs)
}

fn check_object_refs(docs: &BTreeMap<String, Doc>, doc: &str, at: &str, object: &Object) -> Result<()> {
    for (name, field) in &object.properties {
        check_refs(docs, doc, &format!("{at}.{name}"), field)?;
    }
    Ok(())
}

fn check_refs(docs: &BTreeMap<String, Doc>, doc: &str, at: &str, field: &Field) -> Result<()> {
    match field {
        Field::Ref { target, .. } => {
            let definition = resolve_ref(docs, doc, at, target)?;
            if !matches!(
                definition,
                Def::Record { .. } | Def::Object(_) | Def::String { .. } | Def::Token { .. }
            ) {
                bail!("{at}: reference {target} does not name a value type");
            }
            Ok(())
        }
        Field::Array { items, .. } => check_refs(docs, doc, at, items),
        Field::Union { refs, .. } => {
            for target in refs {
                check_object_ref(docs, doc, at, target)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_object_ref(docs: &BTreeMap<String, Doc>, doc: &str, at: &str, target: &str) -> Result<()> {
    if !matches!(resolve_ref(docs, doc, at, target)?, Def::Object(_) | Def::Record { .. }) {
        bail!("{at}: reference {target} does not name an object or record");
    }
    Ok(())
}

fn resolve_ref<'a>(docs: &'a BTreeMap<String, Doc>, doc: &str, at: &str, target: &str) -> Result<&'a Def> {
    let resolved = Target::resolve(doc, target);
    docs.get(&resolved.doc)
        .and_then(|doc| doc.defs.get(&resolved.def))
        .with_context(|| format!("{at}: reference {target} doesn't resolve (vendor the lexicon under lexicons/?)"))
}

/// The file stem a document generates into: `at.motet.pilot.post` → `at_motet_pilot_post`.
#[must_use]
pub fn module_name(nsid: &str) -> String {
    nsid.replace(['.', '-'], "_").to_lowercase()
}
