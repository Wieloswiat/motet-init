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
    /// rejected one. Catches skipped array traversal, ignored fragments and
    /// references incorrectly resolved against the containing document.
    #[test]
    fn dangling_references_are_rejected_at_any_array_depth(
        (reference, target_doc, target_def) in proptest::sample::select(vec![
            ("#entry", "x.y.source", "entry"),
            ("x.y.source#entry", "x.y.source", "entry"),
            ("x.y.target", "x.y.target", "main"),
            ("x.y.target#entry", "x.y.target", "entry"),
        ]),
        depth in 0..8usize,
    ) {
        let dir = tempfile::tempdir().expect("lexicon fixture directory");
        let mut field = json!({"type": "ref", "ref": reference});
        for _ in 0..depth {
            field = json!({"type": "array", "items": field});
        }
        let mut source = json!({
            "main": {"type": "object", "properties": {"value": field}},
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
