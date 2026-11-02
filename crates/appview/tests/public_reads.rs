//! Public reads must reflect admitted archive records, independently of delivery order.

mod support;

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use motet_appview::api::{self, State};
use motet_appview::archive::Archive;
use motet_appview::event::{Action, Event, RecordEvent, SpaceRecordEvent};
use motet_appview::indexer::Indexer;
use motet_appview::{MIGRATOR, projection};
use motet_xrpc::Caller;
use motet_xrpc::server::Authenticate;
use proptest::prelude::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:world";
const AUTHOR: &str = "did:plc:author";
const SCENE: &str = "room";
const POST: &str = "at.motet.pilot.post";
const ADMISSION: &str = "at.motet.pilot.admission";
const CLOSING: &str = "at.motet.pilot.closing";
const SCENES: &str = "at.motet.pilot.scene";
const TIME: &str = "2026-01-01T00:00:00Z";

fn uri(did: &str, collection: &str, key: &str) -> String {
    format!("at://{did}/{collection}/{key}")
}

fn record(did: &str, collection: &str, key: &str, rev: u32, cid: &str, value: Value) -> Event {
    Event::Record(RecordEvent {
        did: did.into(),
        rev: format!("3l{rev:011}"),
        collection: collection.into(),
        rkey: key.into(),
        action: Action::Create,
        cid: Some(cid.into()),
        record: Some(value),
    })
}

fn delete(did: &str, collection: &str, key: &str, rev: u32) -> Event {
    Event::Record(RecordEvent {
        did: did.into(),
        rev: format!("3l{rev:011}"),
        collection: collection.into(),
        rkey: key.into(),
        action: Action::Delete,
        cid: None,
        record: None,
    })
}

fn world(did: &str) -> Event {
    record(
        did,
        "group.opensocial.declaration",
        "self",
        1,
        "world-cid",
        json!({
            "$type": "group.opensocial.declaration",
            "meta": format!("at://{did}/space/group.opensocial.meta/self")
        }),
    )
}

fn meta(did: &str) -> String {
    format!("at://{did}/space/group.opensocial.meta/self")
}

fn space_record(
    space: &str,
    did: &str,
    collection: &str,
    key: &str,
    rev: u32,
    cid: Option<&str>,
    value: Option<Value>,
) -> Event {
    Event::SpaceRecord(SpaceRecordEvent {
        space: space.into(),
        did: did.into(),
        rev: format!("3l{rev:011}"),
        collection: collection.into(),
        rkey: key.into(),
        action: if cid.is_some() { Action::Update } else { Action::Delete },
        cid: cid.map(str::to_owned),
        record: value,
    })
}

fn profile(did: &str, rev: u32, cid: &str, name: &str) -> Event {
    space_record(
        &meta(did),
        did,
        "group.opensocial.profile",
        "self",
        rev,
        Some(cid),
        Some(json!({"$type": "group.opensocial.profile", "displayName": name, "joinPolicy": "invite"})),
    )
}

fn scene(key: &str) -> Event {
    record(WORLD, SCENES, key, 2, &format!("scene-{key}"), scene_value(key))
}

fn scene_value(key: &str) -> Value {
    json!({"$type": SCENES, "title": key, "state": "open", "cast": [AUTHOR], "openedBy": AUTHOR, "createdAt": TIME})
}

fn post_value(world: &str, scene: &str, body: &str) -> Value {
    json!({"$type": POST, "world": world, "scene": scene, "body": body,
        "register": "in-character", "createdAt": TIME})
}

fn post(key: &str, rev: u32, cid: &str, value: Value) -> Event {
    record(AUTHOR, POST, key, rev, cid, value)
}

fn admission(key: &str, post_key: &str, cid: &str, author: &str, scene: &str, epoch: u32, seq: u32) -> Event {
    record(
        WORLD,
        ADMISSION,
        key,
        3 + seq,
        &format!("admit-{key}"),
        json!({
            "$type": ADMISSION, "post": {"uri": uri(AUTHOR, POST, post_key), "cid": cid},
            "author": author, "scene": scene, "place": {"epoch": epoch, "seq": seq},
            "authorityRev": 1, "createdAt": TIME
        }),
    )
}

fn expected_post(key: &str, post_key: &str, cid: &str, epoch: u32, seq: u32, value: &Value) -> Value {
    json!({"admission": {"uri": uri(WORLD, ADMISSION, key), "cid": format!("admit-{key}")},
        "uri": uri(AUTHOR, POST, post_key), "cid": cid, "author": AUTHOR,
        "place": {"epoch": epoch, "seq": seq}, "value": value})
}

fn expected_scene(key: &str) -> Value {
    json!({"uri": uri(WORLD, SCENES, key), "cid": format!("scene-{key}"), "value": scene_value(key)})
}

async fn setup() -> (PgPool, Archive, Indexer, axum::Router) {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let archive = Archive::new(motet_testkit::services::bucket(), "archive");
    let indexer = Indexer::start(pool.clone(), &archive).await.expect("indexer");
    let authenticate: Authenticate = Arc::new(|_| Box::pin(async { Ok(Caller::Anonymous) }));
    let router = api::router(
        Arc::new(State {
            pool: pool.clone(),
            auth: support::oauth(pool.clone()),
            client: support::client(),
        }),
        authenticate,
    )
    .expect("router");
    (pool, archive, indexer, router)
}

async fn query(router: &axum::Router, path: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).expect("request"))
        .await
        .expect("router response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_000_000).await.expect("response body");
    (status, serde_json::from_slice(&bytes).expect("JSON response"))
}

async fn ok(router: &axum::Router, path: &str) -> Value {
    let (status, body) = query(router, path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body
}

fn scene_path(extra: &str) -> String {
    format!("/xrpc/at.motet.pilot.getScene?actor={WORLD}&scene={SCENE}{extra}")
}

async fn domain_views(pool: &PgPool) -> Vec<Vec<Value>> {
    let mut snapshots = Vec::new();
    for view in [
        "public_world_meta",
        "space_records",
        "worlds",
        "scenes",
        "posts",
        "admissions",
        "closings",
        "admitted_posts",
    ] {
        // The identifiers are fixed above, not supplied by a caller.
        let sql = format!("SELECT to_jsonb(v) FROM projection.{view} v ORDER BY to_jsonb(v)::text");
        snapshots.push(
            sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_all(pool)
                .await
                .expect("domain view"),
        );
    }
    snapshots
}

fn replay_streams() -> Vec<Vec<Event>> {
    let mut edited_admission = admission("edit", "p", "new-cid", AUTHOR, SCENE, 1, 2);
    if let Event::Record(r) = &mut edited_admission {
        r.rev = "3l00000000009".into();
        r.record.as_mut().expect("admission value")["replaces"] = json!("post-cid");
    }
    let closing = record(
        WORLD,
        CLOSING,
        "close",
        10,
        "close-cid",
        json!({
            "$type": CLOSING, "admission": {"uri": uri(WORLD, ADMISSION, "a"), "cid": "admit-a"},
            "post": {"uri": uri(AUTHOR, POST, "p"), "cid": "post-cid"}, "scene": SCENE,
            "place": {"epoch": 1, "seq": 2}, "createdAt": TIME
        }),
    );
    let mut edit = post("p", 2, "new-cid", post_value(WORLD, SCENE, "edited"));
    if let Event::Record(r) = &mut edit {
        r.action = Action::Update;
    }
    vec![
        vec![
            world(WORLD),
            scene(SCENE),
            admission("a", "p", "post-cid", AUTHOR, SCENE, 1, 2),
            edited_admission,
            closing,
        ],
        vec![post("p", 1, "post-cid", post_value(WORLD, SCENE, "original")), edit],
    ]
}

// Every legal cross-repository delivery and retry yields the same admitted scene.
// Joining only when the admission arrives would permanently lose early admissions.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]
    #[test]
    #[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
    fn interleaved_redeliveries_and_archive_rebuild_preserve_public_reads(
        delivery in motet_testkit::delivery::deliveries(replay_streams(), 4)
    ) {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(async {
            let (pool, archive, mut indexer, router) = setup().await;
            for event in delivery {
                indexer.ingest(&[event]).await.expect("ingest");
            }
            let expected = json!({"world": {"did": WORLD}, "scene": expected_scene(SCENE),
                "posts": [expected_post("edit", "p", "new-cid", 1, 2, &post_value(WORLD, SCENE, "edited"))]});
            let before = ok(&router, &scene_path("")).await;
            prop_assert_eq!(&before, &expected);
            let views = domain_views(&pool).await;
            projection::rebuild(&pool, &archive).await.expect("archive rebuild");
            prop_assert_eq!(domain_views(&pool).await, views);
            prop_assert_eq!(ok(&router, &scene_path("")).await, expected);
            Ok(())
        })?;
    }
}

fn named_scene_streams() -> Vec<Vec<Event>> {
    let mut final_scene = scene_value(SCENE);
    final_scene["cast"] = json!([AUTHOR, "did:plc:player"]);
    let mut update = record(WORLD, SCENES, SCENE, 4, "scene-new", final_scene);
    if let Event::Record(scene) = &mut update {
        scene.action = Action::Update;
    }
    vec![
        vec![
            Event::Identity {
                did: WORLD.into(),
                handle: Some("world.test".into()),
            },
            world(WORLD),
            scene(SCENE),
            update,
        ],
        vec![
            profile(WORLD, 1, "profile-old", "Old Reach"),
            profile(WORLD, 2, "profile-new", "The Reach"),
        ],
    ]
}

// The current cast, opener and public Space name survive redelivery and an offline rebuild.
// Treating Space revisions as public-repo revisions, or restoring an old scene, loses current data.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]
    #[test]
    #[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
    fn public_profile_and_scene_cast_survive_interleaving_redelivery_and_rebuild(
        delivery in motet_testkit::delivery::deliveries(named_scene_streams(), 4)
    ) {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(async {
            let (pool, archive, mut indexer, router) = setup().await;
            for event in delivery {
                indexer.ingest(&[event]).await.expect("ingest");
            }

            let expected_scene = json!({
                "uri": uri(WORLD, SCENES, SCENE),
                "cid": "scene-new",
                "value": {
                    "$type": SCENES, "title": SCENE, "state": "open",
                    "cast": [AUTHOR, "did:plc:player"], "openedBy": AUTHOR, "createdAt": TIME
                }
            });
            let expected = json!({
                "world": {"did": WORLD, "handle": "world.test", "displayName": "The Reach"},
                "scene": expected_scene, "posts": []
            });
            let before = ok(&router, &scene_path("")).await;
            prop_assert_eq!(&before, &expected);
            let listing = ok(&router, "/xrpc/at.motet.pilot.listScenes?actor=world.test").await;
            prop_assert_eq!(&listing["scenes"], &json!([expected["scene"]]));
            prop_assert_eq!(
                ok(&router, "/xrpc/at.motet.pilot.getWorld?actor=WORLD.TEST").await,
                json!({"did": WORLD, "handle": "world.test", "displayName": "The Reach"})
            );
            let projected: (Value, String) = sqlx::query_as(
                "SELECT cast_dids, opened_by FROM projection.scenes WHERE world_did = $1 AND rkey = $2"
            )
            .bind(WORLD)
            .bind(SCENE)
            .fetch_one(&pool)
            .await
            .expect("scene projection");
            prop_assert_eq!(projected, (json!([AUTHOR, "did:plc:player"]), AUTHOR.to_owned()));
            let views = domain_views(&pool).await;
            projection::rebuild(&pool, &archive).await.expect("archive rebuild");
            prop_assert_eq!(domain_views(&pool).await, views);
            prop_assert_eq!(ok(&router, &scene_path("")).await, expected);
            indexer
                .ingest(&[delete(WORLD, SCENES, SCENE, 5), scene(SCENE)])
                .await
                .expect("scene deletion and stale redelivery");
            let (status, _) = query(&router, &scene_path("")).await;
            prop_assert_eq!(status, StatusCode::NOT_FOUND);
            prop_assert_eq!(
                ok(&router, "/xrpc/at.motet.pilot.listScenes?actor=world.test").await,
                json!({"scenes": []})
            );
            let deleted_views = domain_views(&pool).await;
            projection::rebuild(&pool, &archive).await.expect("rebuild deleted scene");
            prop_assert_eq!(domain_views(&pool).await, deleted_views);
            prop_assert_eq!(query(&router, &scene_path("")).await.0, StatusCode::NOT_FOUND);
            Ok(())
        })?;
    }
}

/// A public World name belongs to its own declared meta Space, not its public repo or another Space.
/// An unscoped join or a stale declaration would expose the wrong or no longer public name.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn public_name_requires_current_matching_meta_and_respects_profile_tombstones() {
    let (pool, archive, mut indexer, router) = setup().await;
    let profile_type = "group.opensocial.profile";
    let other = "did:plc:other";
    let value = |name: &str| json!({"$type": profile_type, "displayName": name, "joinPolicy": "invite"});
    let before_declaration = vec![
        scene(SCENE),
        record(
            WORLD,
            profile_type,
            "self",
            3,
            "repo-profile",
            value("Public repo impostor"),
        ),
        profile(other, 1, "other-profile", "Other World"),
        space_record(
            &meta(WORLD),
            other,
            profile_type,
            "self",
            1,
            Some("foreign-repo"),
            Some(value("Foreign repo")),
        ),
        space_record(
            &meta(WORLD),
            WORLD,
            profile_type,
            "other",
            1,
            Some("wrong-key"),
            Some(value("Wrong key")),
        ),
    ];
    indexer
        .ingest(&before_declaration)
        .await
        .expect("records without a declaration");
    let world_path = format!("/xrpc/at.motet.pilot.getWorld?actor={WORLD}");
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));

    indexer.ingest(&[world(WORLD)]).await.expect("public declaration");
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));
    indexer
        .ingest(&[
            profile(WORLD, 1, "name-old", "Old Reach"),
            profile(WORLD, 2, "name-new", "The Reach"),
            profile(WORLD, 1, "name-old", "Old Reach"),
        ])
        .await
        .expect("current name and stale redelivery");
    assert_eq!(
        ok(&router, &world_path).await,
        json!({"did": WORLD, "displayName": "The Reach"})
    );
    assert_eq!(ok(&router, &scene_path("")).await["world"]["displayName"], "The Reach");

    let removed = space_record(&meta(WORLD), WORLD, profile_type, "self", 3, None, None);
    indexer
        .ingest(&[removed, profile(WORLD, 2, "name-new", "The Reach")])
        .await
        .expect("profile deletion and stale redelivery");
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));

    indexer
        .ingest(&[profile(WORLD, 4, "name-restored", "Restored Reach")])
        .await
        .expect("restored profile");
    assert_eq!(ok(&router, &world_path).await["displayName"], "Restored Reach");
    indexer
        .ingest(&[
            space_record(
                &meta(WORLD),
                WORLD,
                profile_type,
                "self",
                5,
                Some("invalid-name"),
                Some(json!({"$type": profile_type, "displayName": 17, "joinPolicy": "invite"})),
            ),
            profile(WORLD, 4, "name-restored", "Restored Reach"),
        ])
        .await
        .expect("invalid newer name and stale redelivery");
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));
    indexer
        .ingest(&[profile(WORLD, 6, "name-final", "The Reach")])
        .await
        .expect("corrected profile");
    assert_eq!(ok(&router, &world_path).await["displayName"], "The Reach");

    indexer
        .ingest(&[
            record(
                WORLD,
                "group.opensocial.declaration",
                "self",
                8,
                "foreign-declaration",
                json!({"$type": "group.opensocial.declaration", "meta": meta(other)}),
            ),
            space_record(
                &meta(other),
                WORLD,
                profile_type,
                "self",
                7,
                Some("foreign-space"),
                Some(value("Foreign Space")),
            ),
        ])
        .await
        .expect("foreign meta declaration");
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));
    indexer
        .ingest(&[delete(WORLD, "group.opensocial.declaration", "self", 9)])
        .await
        .expect("declaration removed");
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));
    let views = domain_views(&pool).await;
    projection::rebuild(&pool, &archive).await.expect("archive rebuild");
    assert_eq!(domain_views(&pool).await, views);
    assert_eq!(ok(&router, &world_path).await, json!({"did": WORLD}));
}

/// A post becomes visible only when its exact version, author, World and scene agree.
/// Joining on URI alone leaks mismatched, unadmitted or not-yet-arrived content.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn only_matching_admitted_versions_are_public() {
    let (_, _, mut indexer, router) = setup().await;
    let events = vec![
        world(WORLD),
        scene(SCENE),
        admission("good", "good", "good-cid", AUTHOR, SCENE, 0, 1),
        admission("cid", "cid", "other-cid", AUTHOR, SCENE, 0, 2),
        admission("author", "author", "author-cid", "did:plc:someoneelse", SCENE, 0, 3),
        admission("world", "world", "world-cid", AUTHOR, SCENE, 0, 4),
        admission("scene", "scene", "scene-cid", AUTHOR, SCENE, 0, 5),
    ];
    indexer.ingest(&events).await.expect("admissions first");
    assert_eq!(ok(&router, &scene_path("")).await["posts"], json!([]));
    let value = post_value(WORLD, SCENE, "good");
    indexer
        .ingest(&[
            post("good", 1, "good-cid", value.clone()),
            post("cid", 2, "cid-cid", post_value(WORLD, SCENE, "wrong cid")),
            post("author", 3, "author-cid", post_value(WORLD, SCENE, "wrong author")),
            post(
                "world",
                4,
                "world-cid",
                post_value("did:plc:other", SCENE, "wrong world"),
            ),
            post("scene", 5, "scene-cid", post_value(WORLD, "elsewhere", "wrong scene")),
            post("missing", 6, "missing-cid", post_value(WORLD, SCENE, "not admitted")),
        ])
        .await
        .expect("posts");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("good", "good", "good-cid", 0, 1, &value)])
    );
}

/// An edit does not replace an admitted version until its own admission arrives.
/// A current-record join would silently substitute unadmitted edits.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn edits_keep_the_admitted_bytes_until_superseded() {
    let (_, _, mut indexer, router) = setup().await;
    let old = post_value(WORLD, SCENE, "old");
    let new = post_value(WORLD, SCENE, "new");
    let mut update = post("p", 2, "new-cid", new.clone());
    if let Event::Record(r) = &mut update {
        r.action = Action::Update;
    }
    indexer
        .ingest(&[
            world(WORLD),
            scene(SCENE),
            post("p", 1, "old-cid", old.clone()),
            admission("old", "p", "old-cid", AUTHOR, SCENE, 0, 1),
            update,
        ])
        .await
        .expect("edit");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("old", "p", "old-cid", 0, 1, &old)])
    );
    let mut edit = admission("new", "p", "new-cid", AUTHOR, SCENE, 0, 1);
    if let Event::Record(r) = &mut edit {
        r.record.as_mut().expect("value")["replaces"] = json!("old-cid");
        r.rev = "3l00000000009".into();
    }
    indexer.ingest(&[edit]).await.expect("edit admission");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("new", "p", "new-cid", 0, 1, &new)])
    );
}

/// A closing survives deletion, byte-identical recreation and stale deliveries.
/// Deduplicating operations by CID would resurrect the closed place.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn closing_and_recreation_require_a_new_place() {
    let (_, _, mut indexer, router) = setup().await;
    let value = post_value(WORLD, SCENE, "same bytes");
    let original = post("p", 1, "same-cid", value.clone());
    let deletion = delete(AUTHOR, POST, "p", 2);
    indexer
        .ingest(&[
            world(WORLD),
            scene(SCENE),
            original.clone(),
            admission("old", "p", "same-cid", AUTHOR, SCENE, 0, 1),
            deletion.clone(),
        ])
        .await
        .expect("delete");
    let closing = record(
        WORLD,
        CLOSING,
        "close",
        10,
        "close-cid",
        json!({
            "$type": CLOSING, "admission": {"uri": uri(WORLD, ADMISSION, "old"), "cid": "admit-old"},
            "post": {"uri": uri(AUTHOR, POST, "p"), "cid": "same-cid"}, "scene": SCENE,
            "place": {"epoch": 0, "seq": 1}, "createdAt": TIME
        }),
    );
    indexer
        .ingest(&[closing, post("p", 3, "same-cid", value.clone()), original, deletion])
        .await
        .expect("recreate and stale deliveries");
    assert_eq!(ok(&router, &scene_path("")).await["posts"], json!([]));
    let mut fresh = admission("fresh", "p", "same-cid", AUTHOR, SCENE, 0, 2);
    if let Event::Record(r) = &mut fresh {
        r.rev = "3l00000000011".into();
    }
    indexer.ingest(&[fresh]).await.expect("new admission");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("fresh", "p", "same-cid", 0, 2, &value)])
    );
}

/// Public queries resolve DIDs and handles and paginate in stable key order.
/// Ordering by arrival or timestamp, or emitting a cursor on the last page, loses or repeats rows.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn public_queries_resolve_world_and_paginate_by_scene_and_place() {
    let (_, _, mut indexer, router) = setup().await;
    let mut value_a = post_value(WORLD, SCENE, "first");
    let mut value_b = post_value(WORLD, SCENE, "second");
    let mut value_c = post_value(WORLD, SCENE, "third");
    value_a["createdAt"] = json!("2026-01-03T00:00:00Z");
    value_b["createdAt"] = json!("2026-01-02T00:00:00Z");
    value_c["createdAt"] = json!("2026-01-01T00:00:00Z");
    indexer
        .ingest(&[
            world(WORLD),
            Event::Identity {
                did: WORLD.into(),
                handle: Some("world.test".into()),
            },
            scene("zoo"),
            scene(SCENE),
            scene("alpha"),
            post("c", 1, "cid-c", value_c.clone()),
            post("a", 2, "cid-a", value_a.clone()),
            post("b", 3, "cid-b", value_b.clone()),
            admission("c", "c", "cid-c", AUTHOR, SCENE, 2, 0),
            admission("a", "a", "cid-a", AUTHOR, SCENE, 0, 9),
            admission("b", "b", "cid-b", AUTHOR, SCENE, 1, 0),
        ])
        .await
        .expect("records");
    assert_eq!(
        ok(&router, "/xrpc/at.motet.pilot.getWorld?actor=world.test").await,
        json!({"did": WORLD, "handle": "world.test"})
    );
    assert_eq!(
        ok(&router, &format!("/xrpc/at.motet.pilot.getWorld?actor={WORLD}")).await,
        json!({"did": WORLD, "handle": "world.test"})
    );
    assert_eq!(
        ok(&router, "/xrpc/at.motet.pilot.listScenes?actor=world.test&limit=2").await,
        json!({"scenes": [expected_scene("alpha"), expected_scene(SCENE)], "cursor": SCENE})
    );
    assert_eq!(
        ok(
            &router,
            "/xrpc/at.motet.pilot.listScenes?actor=world.test&limit=2&cursor=room"
        )
        .await,
        json!({"scenes": [expected_scene("zoo")]})
    );
    assert_eq!(
        ok(&router, &scene_path("&limit=2")).await,
        json!({
            "world": {"did": WORLD, "handle": "world.test"}, "scene": expected_scene(SCENE),
            "posts": [expected_post("a", "a", "cid-a", 0, 9, &value_a),
                      expected_post("b", "b", "cid-b", 1, 0, &value_b)], "cursor": "1:0"
        })
    );
    assert_eq!(
        ok(&router, &scene_path("&limit=2&cursor=1:0")).await,
        json!({
            "world": {"did": WORLD, "handle": "world.test"}, "scene": expected_scene(SCENE),
            "posts": [expected_post("c", "c", "cid-c", 2, 0, &value_c)]
        })
    );
}

/// Conflicting handle claims cannot redirect readers to an arbitrary World.
/// Stale identity events can leave two DIDs claiming the same case-insensitive handle.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn ambiguous_handles_fail_closed_but_dids_remain_resolvable() {
    let (_, _, mut indexer, router) = setup().await;
    let other = "did:plc:other";
    indexer
        .ingest(&[
            scene(SCENE),
            record(other, SCENES, SCENE, 1, "other-scene", scene_value(SCENE)),
            Event::Identity {
                did: WORLD.into(),
                handle: Some("World.Test".into()),
            },
            Event::Identity {
                did: other.into(),
                handle: Some("world.test".into()),
            },
        ])
        .await
        .expect("conflicting identity events");
    for handle in ["world.test", "WORLD.TEST"] {
        let (status, body) = query(&router, &format!("/xrpc/at.motet.pilot.getWorld?actor={handle}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{handle}: {body}");
        assert_eq!(body["error"], "NotFound");
    }
    assert_eq!(
        ok(&router, &format!("/xrpc/at.motet.pilot.getWorld?actor={WORLD}")).await,
        json!({"did": WORLD, "handle": "World.Test"})
    );
    assert_eq!(
        ok(&router, &format!("/xrpc/at.motet.pilot.getWorld?actor={other}")).await,
        json!({"did": other, "handle": "world.test"})
    );
}

/// Unknown targets and invalid bounds never become empty successful pages.
/// Treating missing Worlds as empty or accepting oversized limits hides client errors.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn missing_targets_and_invalid_pagination_are_rejected() {
    let (_, _, mut indexer, router) = setup().await;
    indexer.ingest(&[world(WORLD), scene(SCENE)]).await.expect("records");
    for path in [
        "/xrpc/at.motet.pilot.getWorld?actor=did:plc:unknown",
        "/xrpc/at.motet.pilot.listScenes?actor=did:plc:unknown",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:unknown&scene=room",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:world&scene=missing",
    ] {
        let (status, body) = query(&router, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(body["error"], "NotFound");
    }
    for path in [
        "/xrpc/at.motet.pilot.listScenes?actor=did:plc:world&limit=0",
        "/xrpc/at.motet.pilot.listScenes?actor=did:plc:world&limit=101",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:world&scene=room&limit=0",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:world&scene=room&limit=101",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:world&scene=room&cursor=-1:0",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:world&scene=room&cursor=1:9223372036854775808",
        "/xrpc/at.motet.pilot.getScene?actor=did:plc:world&scene=room&cursor=bad",
    ] {
        let (status, body) = query(&router, path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
    }
}

/// Public scenes reveal their World even when its group declaration is private.
/// Requiring a public declaration would make tenor-created Worlds undiscoverable.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn public_scene_discovers_a_world_without_a_declaration() {
    let (_, _, mut indexer, router) = setup().await;
    indexer.ingest(&[scene(SCENE)]).await.expect("public scene");
    assert_eq!(
        ok(&router, &format!("/xrpc/at.motet.pilot.getWorld?actor={WORLD}")).await,
        json!({"did": WORLD})
    );
    assert_eq!(
        ok(&router, &format!("/xrpc/at.motet.pilot.listScenes?actor={WORLD}")).await,
        json!({"scenes": [expected_scene(SCENE)]})
    );
}

/// Unspecified limits cap each public list at 50 and allow a continuation.
/// Fetching all rows or forgetting the extra row defeats the public bound.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn default_pages_are_bounded_for_every_list() {
    let (_, _, mut indexer, router) = setup().await;
    let mut events = vec![world(WORLD), scene(SCENE)];
    for n in 0..51 {
        let key = format!("k{n:02}");
        events.push(scene(&key));
        events.push(post(&key, n + 1, &format!("cid-{key}"), post_value(WORLD, SCENE, &key)));
        events.push(admission(&key, &key, &format!("cid-{key}"), AUTHOR, SCENE, 0, n));
    }
    indexer.ingest(&events).await.expect("records");
    let scenes = ok(&router, &format!("/xrpc/at.motet.pilot.listScenes?actor={WORLD}")).await;
    assert_eq!(scenes["scenes"].as_array().expect("scenes").len(), 50);
    assert_eq!(scenes["cursor"], "k49");
    let posts = ok(&router, &scene_path("")).await;
    assert_eq!(posts["posts"].as_array().expect("posts").len(), 50);
    assert_eq!(posts["cursor"], "0:49");
}

/// Another World's records cannot add or close posts in this World's scene.
/// Joining on scene key or admission reference without the World DID crosses tenants.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn foreign_world_records_cannot_change_a_scene() {
    let (_, _, mut indexer, router) = setup().await;
    let value = post_value(WORLD, SCENE, "ours");
    let own = admission("own", "p", "cid-p", AUTHOR, SCENE, 0, 1);
    indexer
        .ingest(&[world(WORLD), scene(SCENE), post("p", 1, "cid-p", value.clone()), own])
        .await
        .expect("own records");
    let before = ok(&router, &scene_path("")).await;
    let foreign = "did:plc:foreign";
    let foreign_scene = record(foreign, SCENES, SCENE, 1, "foreign-scene", scene_value(SCENE));
    let mut foreign_admission = admission("foreign", "p", "cid-p", AUTHOR, SCENE, 0, 2);
    if let Event::Record(r) = &mut foreign_admission {
        r.did = foreign.into();
    }
    let foreign_closing = record(
        foreign,
        CLOSING,
        "foreign-close",
        3,
        "foreign-close-cid",
        json!({
            "$type": CLOSING, "admission": {"uri": uri(WORLD, ADMISSION, "own"), "cid": "admit-own"},
            "post": {"uri": uri(AUTHOR, POST, "p"), "cid": "cid-p"}, "scene": SCENE,
            "place": {"epoch": 0, "seq": 1}, "createdAt": TIME
        }),
    );
    indexer
        .ingest(&[foreign_scene, foreign_admission, foreign_closing])
        .await
        .expect("foreign records");
    assert_eq!(ok(&router, &scene_path("")).await, before);
    assert_eq!(
        before["posts"],
        json!([expected_post("own", "p", "cid-p", 0, 1, &value)])
    );
}

/// A newer admission reserves its place even before its post version arrives.
/// Ranking only joined posts would incorrectly revive the superseded version.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn missing_new_version_does_not_revive_superseded_admission() {
    let (_, _, mut indexer, router) = setup().await;
    let old = post_value(WORLD, SCENE, "old");
    let new = post_value(WORLD, SCENE, "new");
    indexer
        .ingest(&[
            world(WORLD),
            scene(SCENE),
            post("p", 1, "old-cid", old.clone()),
            admission("old", "p", "old-cid", AUTHOR, SCENE, 0, 1),
        ])
        .await
        .expect("old admission");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("old", "p", "old-cid", 0, 1, &old)])
    );
    let mut newer = admission("new", "p", "new-cid", AUTHOR, SCENE, 0, 1);
    if let Event::Record(r) = &mut newer {
        r.rev = "3l00000000009".into();
        r.record.as_mut().expect("admission value")["replaces"] = json!("old-cid");
    }
    indexer.ingest(&[newer]).await.expect("new admission");
    assert_eq!(ok(&router, &scene_path("")).await["posts"], json!([]));
    let mut update = post("p", 2, "new-cid", new.clone());
    if let Event::Record(r) = &mut update {
        r.action = Action::Update;
    }
    indexer.ingest(&[update]).await.expect("new version");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("new", "p", "new-cid", 0, 1, &new)])
    );
}

/// Invalid place values cannot break scene reads or admit content.
/// Casting malformed JSON directly can abort the whole query or truncate a fraction.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn invalid_places_do_not_break_or_leak_into_scene_reads() {
    let (_, _, mut indexer, router) = setup().await;
    let value = post_value(WORLD, SCENE, "valid");
    let mut events = vec![
        world(WORLD),
        scene(SCENE),
        post("good", 1, "good-cid", value.clone()),
        admission("good", "good", "good-cid", AUTHOR, SCENE, 0, 1),
    ];
    for (n, place) in [
        json!({"epoch": -1, "seq": 0}),
        json!({"epoch": 0, "seq": 9_223_372_036_854_775_808_u64}),
        json!({"epoch": 1.5, "seq": 0}),
        json!({"epoch": 0, "seq": "2"}),
        json!({"epoch": 0}),
        json!("not a place"),
    ]
    .into_iter()
    .enumerate()
    {
        let key = format!("bad{n}");
        let cid = format!("cid-{key}");
        events.push(post(
            &key,
            2 + u32::try_from(n).expect("small index"),
            &cid,
            post_value(WORLD, SCENE, &key),
        ));
        let mut bad = admission(
            &key,
            &key,
            &cid,
            AUTHOR,
            SCENE,
            0,
            10 + u32::try_from(n).expect("small index"),
        );
        if let Event::Record(r) = &mut bad {
            r.record.as_mut().expect("admission value")["place"] = place;
        }
        events.push(bad);
    }
    indexer.ingest(&events).await.expect("invalid records are archived");
    assert_eq!(
        ok(&router, &scene_path("")).await["posts"],
        json!([expected_post("good", "good", "good-cid", 0, 1, &value)])
    );
}

/// Unknown fields in a valid closing do not prevent it closing its admission.
/// Comparing entire place objects would leave closed posts visible after extensions.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn closing_with_an_extended_place_still_closes_the_admission() {
    let (_, _, mut indexer, router) = setup().await;
    indexer
        .ingest(&[
            world(WORLD),
            scene(SCENE),
            post("p", 1, "cid-p", post_value(WORLD, SCENE, "closed")),
            admission("a", "p", "cid-p", AUTHOR, SCENE, 0, 1),
        ])
        .await
        .expect("admission");
    let closing = record(
        WORLD,
        CLOSING,
        "close",
        10,
        "close-cid",
        json!({
            "$type": CLOSING, "admission": {"uri": uri(WORLD, ADMISSION, "a"), "cid": "admit-a"},
            "post": {"uri": uri(AUTHOR, POST, "p"), "cid": "cid-p"}, "scene": SCENE,
            "place": {"epoch": 0, "seq": 1, "extension": "ignored"}, "createdAt": TIME
        }),
    );
    indexer.ingest(&[closing]).await.expect("closing");
    assert_eq!(ok(&router, &scene_path("")).await["posts"], json!([]));
}
