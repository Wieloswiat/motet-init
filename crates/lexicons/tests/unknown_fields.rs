//! Records written by newer clients survive a round trip through our types.

use motet_lexicons::at_motet_pilot_post::Post;
use motet_lexicons::com_atproto_label_defs::Label;
use motet_lexicons::group_opensocial_createreport::CreateReportInput;
use motet_lexicons::group_opensocial_defs::{SubjectView, SubjectViewSubject, SubjectViewSubjectKnown};
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

fn future_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i32>().prop_map(|value| json!(value)),
        "[a-zA-Z0-9]{0,16}".prop_map(Value::String),
    ]
    .prop_recursive(3, 32, 5, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            proptest::collection::btree_map("[a-z]{1,8}", inner, 0..5)
                .prop_map(|entries| Value::Object(entries.into_iter().collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

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

    /// A union dispatches on `$type`, not on fields that happen to fit a known
    /// variant. Dropping the tag or unknown nested values breaks forwarding.
    #[test]
    fn tagged_unions_keep_known_and_future_variants(
        variant in 0..3u8,
        future in future_value(),
    ) {
        let tag = match variant {
            0 => "group.opensocial.defs#accountSubject",
            1 => "group.opensocial.defs#recordSubject",
            _ => "group.opensocial.defs#futureSubject",
        };
        let subject = json!({
            "$type": tag,
            "did": "did:plc:subject",
            "uri": "at://did:plc:world/space/group.opensocial.meta/self/did:plc:world/group.opensocial.rule/rule",
            "future": future,
        });
        let value = json!({
            "subject": subject,
            "status": "open",
            "createdAt": "2026-10-19T00:00:00Z",
            "updatedAt": "2026-10-19T00:00:00Z",
            "reportCount": 1,
            "future": future,
        });
        let view: SubjectView = serde_json::from_value(value.clone()).expect("tagged subject decodes");
        prop_assert!(
            matches!(
                (&view.subject, variant),
                (SubjectViewSubject::Known(SubjectViewSubjectKnown::AccountSubject(_)), 0)
                    | (SubjectViewSubject::Known(SubjectViewSubjectKnown::RecordSubject(_)), 1)
                    | (SubjectViewSubject::Unknown { .. }, 2)
            ),
            "union dispatches by its tag"
        );
        let mut untagged = value.clone();
        untagged["subject"].as_object_mut().expect("subject is an object").remove("$type");
        prop_assert!(serde_json::from_value::<SubjectView>(untagged).is_err());
        prop_assert_eq!(serde_json::to_value(view).expect("subject encodes"), value);

        let request = json!({
            "group": "did:plc:world",
            "subject": subject,
            "reasonType": "future.example.report#reason",
            "future": future,
        });
        let input: CreateReportInput = serde_json::from_value(request.clone()).expect("RPC input decodes");
        prop_assert_eq!(serde_json::to_value(input).expect("RPC input encodes"), request);
    }

    /// Label signatures use DAG-JSON bytes, not JSON number arrays.
    /// A `Vec<u8>` mapping would refuse the network form or change it on output.
    #[test]
    fn signature_bytes_keep_the_protocol_json_encoding(bytes in "([A-Za-z0-9+/]{4}){1,4}") {
        let value = json!({
            "src": "did:plc:labeler",
            "uri": "at://did:plc:subject",
            "val": "future-label",
            "cts": "2026-10-19T00:00:00Z",
            "sig": { "$bytes": bytes },
        });
        let label: Label = serde_json::from_value(value.clone()).expect("signature decodes");
        prop_assert_eq!(serde_json::to_value(label).expect("signature encodes"), value);
    }
}
