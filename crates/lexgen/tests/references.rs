//! Reference validation protects consumers from schemas whose types cannot be resolved.

use std::path::Path;

use proptest::prelude::*;
use serde_json::{Value, json};

fn write_doc(dir: &Path, id: &str, defs: &Value) {
    let path = dir.join(id.replace('.', "/")).with_extension("json");
    std::fs::create_dir_all(path.parent().expect("document path has namespace directories"))
        .expect("namespace directory");
    let doc = json!({"lexicon": 1, "id": id, "defs": defs});
    std::fs::write(path, serde_json::to_vec(&doc).expect("JSON values serialize")).expect("write lexicon");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Removing a referenced definition must turn an accepted schema into a
    /// rejected one. Catches skipped union members, RPC bodies, array traversal,
    /// ignored fragments and references resolved against the wrong document.
    #[test]
    fn dangling_references_are_rejected_through_unions_arrays_and_rpc_bodies(
        (reference, target_doc, target_def) in proptest::sample::select(vec![
            ("#entry", "x.y.source", "entry"),
            ("x.y.source#entry", "x.y.source", "entry"),
            ("x.y.target", "x.y.target", "main"),
            ("x.y.target#entry", "x.y.target", "entry"),
        ]),
        depth in 0..8usize,
        union in any::<bool>(),
        position in 0..8u8,
    ) {
        let dir = tempfile::tempdir().expect("lexicon fixture directory");
        let mut field = if union {
            json!({"type": "union", "refs": ["#available", reference]})
        } else {
            json!({"type": "ref", "ref": reference})
        };
        for _ in 0..depth {
            field = json!({"type": "array", "items": field});
        }
        let object = json!({"type": "object", "properties": {"value": field}});
        let body = json!({"encoding": "application/json", "schema": object});
        let ref_body = json!({"encoding": "application/json", "schema": {"type": "ref", "ref": reference}});
        let main = match position {
            0 => object,
            1 => json!({"type": "record", "key": "tid", "record": object}),
            2 => json!({"type": "query", "parameters": {"type": "params", "properties": {"value": field}}}),
            3 => json!({"type": "query", "output": body}),
            4 => json!({"type": "procedure", "input": body}),
            5 => json!({"type": "procedure", "output": body}),
            6 => json!({"type": "query", "output": ref_body}),
            _ => json!({"type": "procedure", "input": ref_body}),
        };
        let mut source = json!({
            "main": main,
            "available": {"type": "object"},
            "entry": {"type": "object"},
        });
        let mut target = json!({"main": {"type": "object"}, "entry": {"type": "object"}});
        write_doc(dir.path(), "x.y.source", &source);
        write_doc(dir.path(), "x.y.target", &target);
        prop_assert!(motet_lexgen::load(dir.path()).is_ok());

        let defs = if target_doc == "x.y.source" { &mut source } else { &mut target };
        defs.as_object_mut().expect("fixture definitions are objects").remove(target_def);
        write_doc(dir.path(), target_doc, defs);
        prop_assert!(motet_lexgen::load(dir.path()).is_err());
    }
}
