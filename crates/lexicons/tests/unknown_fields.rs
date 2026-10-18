//! Records written by newer clients survive a round trip through our types.

use motet_lexicons::at_motet_pilot_post::Post;
use proptest::prelude::*;
use serde_json::{Value, json};

fn base() -> Value {
    json!({
        "$type": "at.motet.pilot.post",
        "world": "did:plc:world",
        "scene": "3kscene",
        "register": "in-character",
        "body": "= Hello",
        "createdAt": "2026-10-18T23:00:00Z",
    })
}

proptest! {
    #[test]
    fn unknown_fields_are_written_back_unchanged(
        extra in proptest::collection::btree_map("[a-z]{1,8}Future", any::<i32>(), 0..5)
    ) {
        let mut value = base();
        for (k, v) in &extra {
            value[k] = json!(v);
        }
        let post: Post = serde_json::from_value(value.clone()).expect("decodes");
        prop_assert_eq!(serde_json::to_value(&post).expect("encodes"), value);
    }
}
