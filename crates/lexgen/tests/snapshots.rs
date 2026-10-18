//! The generator's output for the real lexicons, reviewed as snapshots.
//! A change here is a change to the generated API: review it like one.

use std::path::Path;

#[test]
fn generated_code_for_motet_lexicons() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lexicons");
    let docs = motet_lexgen::load(&dir).expect("lexicons load");
    for (file, contents) in motet_lexgen::rust::generate(&docs).expect("rust").into_iter().chain(motet_lexgen::ts::generate(&docs).expect("ts")) {
        insta::assert_snapshot!(file.clone(), contents);
    }
}

#[test]
fn dangling_references_are_rejected() {
    let dir = std::env::temp_dir().join(format!("lexgen-dangling-{}", std::process::id()));
    let nested = dir.join("x/y");
    std::fs::create_dir_all(&nested).expect("tmp dir");
    std::fs::write(
        nested.join("z.json"),
        r#"{"lexicon":1,"id":"x.y.z","defs":{"main":{"type":"object","properties":{"a":{"type":"ref","ref":"x.y.missing"}}}}}"#,
    )
    .expect("write");
    let err = motet_lexgen::load(&dir).expect_err("dangling ref");
    assert!(err.to_string().contains("doesn't resolve"), "{err}");
    std::fs::remove_dir_all(dir).ok();
}
