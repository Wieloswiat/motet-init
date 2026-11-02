//! A fresh OAuth account can create a public World and see its admitted post.
//!
//! This test needs the full `devenv up` stack, local TLS and DNS from
//! `tls-setup`, and the service origins exported by the dev shell.

mod oauth_support;

use std::time::Duration;

use atproto_identity::key::{KeyType, generate_key, sign, to_public};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::header::{COOKIE, ORIGIN};
use reqwest::{Response, StatusCode};
use serde_json::{Value, json};
use tokio::time::{Instant, sleep_until, timeout_at};
use url::Url;
use uuid::Uuid;

use oauth_support::OAuthSession;

async fn successful_json(response: Response, stage: &str) -> Value {
    let status = response.status();
    let body = response.text().await.expect("read the response body");
    assert_eq!(status, StatusCode::OK, "{stage}: HTTP {status}: {body}");
    serde_json::from_str(&body).unwrap_or_else(|error| panic!("{stage}: invalid JSON: {error}; body: {body}"))
}

async fn procedure(session: &OAuthSession, method: &str, body: &Value) -> Value {
    let url = session
        .app
        .join(&format!("/xrpc/{method}"))
        .expect("the AppView procedure URL is valid");
    // A queued createPost can wait 30 seconds before reporting pending.
    let response = session
        .client
        .post(url)
        .timeout(Duration::from_secs(60))
        .header(COOKIE, &session.session_cookie)
        .header(ORIGIN, session.app.origin().ascii_serialization())
        .json(body)
        .send()
        .await
        .unwrap_or_else(|error| panic!("{method} request failed: {error}"));
    successful_json(response, method).await
}

fn string<'a>(value: &'a Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{field} must be a string: {value}"))
}

// Relay, Tap, and the public meta Space can deliver independently. A dropped
// admission, mismatched version, missing profile, or broken authenticated
// service grant must not leave the owner's World or post incomplete.
#[tokio::test]
#[ignore = "integration: full HTTPS PDS, AppView, tenor, relay, Tap, PLC, PostgreSQL and S3 stack"]
#[expect(
    clippy::too_many_lines,
    reason = "the ordered journey shares state across every step"
)]
async fn oauth_owner_creates_a_world_and_sees_the_exact_post_in_first_place() {
    let session = oauth_support::create_session(None).await;
    let current = session
        .client
        .get(
            session
                .app
                .join("/xrpc/at.motet.pilot.auth.getSession")
                .expect("the AppView session URL is valid"),
        )
        .header(COOKIE, &session.session_cookie)
        .send()
        .await
        .expect("read the new AppView session");
    let current = successful_json(current, "getSession").await;
    assert_eq!(current["did"], session.did);
    assert_eq!(current["handle"], session.handle);

    let rotation = generate_key(KeyType::P256Private).expect("generate the owner's private P-256 rotation key");
    let public_rotation = to_public(&rotation).expect("derive the owner's public rotation key");
    let domains = std::env::var("MOTET_TENOR_HANDLE_DOMAINS").expect("the tenor handle domains must be configured");
    let domain = domains.split(',').next().expect("the tenor has a handle domain").trim();
    assert!(domain.starts_with('.'), "the tenor handle domain must start with a dot");
    let run_id = Uuid::now_v7().simple().to_string();
    let handle = format!("world{run_id}{domain}");
    let world_name = "Live stack World";
    let reserved = procedure(
        &session,
        "at.motet.pilot.beginWorld",
        &json!({
            "handle": handle,
            "name": world_name,
            "visibility": "public",
            "rotationKey": public_rotation.to_string(),
        }),
    )
    .await;

    let signing_bytes = URL_SAFE_NO_PAD
        .decode(string(&reserved, "signingBytes"))
        .expect("the reservation contains unpadded base64url signing bytes");
    let signature = sign(&rotation, &signing_bytes).expect("the owner signs the reserved PLC genesis");
    let mut operation = reserved["operation"].clone();
    operation
        .as_object_mut()
        .expect("the unsigned PLC operation is an object")
        .insert("sig".into(), URL_SAFE_NO_PAD.encode(signature).into());
    let created = procedure(
        &session,
        "at.motet.pilot.finishWorld",
        &json!({"reservation": string(&reserved, "reservation"), "signedOperation": operation}),
    )
    .await;
    let world = string(&created, "did");
    assert!(world.starts_with("did:plc:"), "the World has a PLC DID: {created}");
    assert_eq!(string(&created, "handle"), handle);

    // A lost response must not create another scene when the caller retries.
    // Minting a new operation ID in the AppView would return a different URI.
    let title = "The first scene";
    let open_input = json!({"op": Uuid::now_v7().to_string(), "world": world, "title": title});
    let opened = procedure(&session, "at.motet.pilot.openScene", &open_input).await;
    let retried = procedure(&session, "at.motet.pilot.openScene", &open_input).await;
    assert_eq!(opened["uri"], retried["uri"], "retry returns the original scene");
    assert_eq!(opened["scene"], retried["scene"]);
    let tenor = std::env::var("MOTET_APPVIEW_TENOR_URL").expect("the tenor origin must be configured");
    let records = session
        .client
        .get(format!("{tenor}/xrpc/com.atproto.repo.listRecords"))
        .query(&[("repo", world), ("collection", "at.motet.pilot.scene")])
        .send()
        .await
        .expect("list the World's scene records");
    let records = successful_json(records, "list scene records").await;
    let scenes = records["records"].as_array().expect("scene records are an array");
    assert_eq!(scenes.len(), 1, "retry must publish exactly one scene: {records}");
    assert_eq!(scenes[0]["uri"], opened["uri"]);
    let scene = string(&opened, "scene");
    let scene_uri = string(&opened, "uri");
    let scene_cid = string(&opened, "cid");
    let text = format!("A first post from the live journey {run_id}");
    let published = procedure(
        &session,
        "at.motet.pilot.createPost",
        &json!({"world": world, "scene": scene, "body": text, "register": "in-character"}),
    )
    .await;
    match string(&published, "status") {
        "admitted" => assert!(
            published["epoch"].as_u64().is_some() && published["seq"].as_u64().is_some(),
            "an admission identifies its place: {published}"
        ),
        "pending" => {}
        status => panic!("post admission returned {status}: {published}"),
    }
    let post_uri = string(&published, "uri");
    let post_cid = string(&published, "cid");
    let post_prefix = format!("at://{}/at.motet.pilot.post/", session.did);
    let post_key = post_uri
        .strip_prefix(&post_prefix)
        .filter(|key| !key.is_empty() && !key.contains('/'))
        .expect("the published post belongs to the signed-in author's repo");

    // The author's PDS is an independent oracle for the full record value,
    // including the creation timestamp that the AppView assigned.
    let pds = std::env::var("MOTET_PDS_RS_URL").expect("the PDS origin must be configured");
    let pds = Url::parse(&pds).expect("the PDS origin is a URL");
    let record = session
        .client
        .get(
            pds.join("/xrpc/com.atproto.repo.getRecord")
                .expect("the PDS record URL is valid"),
        )
        .query(&[
            ("repo", session.did.as_str()),
            ("collection", "at.motet.pilot.post"),
            ("rkey", post_key),
            ("cid", post_cid),
        ])
        .send()
        .await
        .expect("read the published post from the author's PDS");
    let record = successful_json(record, "PDS getRecord").await;
    assert_eq!(string(&record, "uri"), post_uri);
    assert_eq!(string(&record, "cid"), post_cid);
    assert_eq!(record["value"]["$type"], "at.motet.pilot.post");
    assert_eq!(record["value"]["world"], world);
    assert_eq!(record["value"]["scene"], scene);
    assert_eq!(record["value"]["body"], text);
    assert_eq!(record["value"]["register"], "in-character");
    assert!(
        record["value"]["createdAt"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    let mut query_url = session
        .app
        .join("/xrpc/at.motet.pilot.getScene")
        .expect("the AppView scene URL is valid");
    query_url
        .query_pairs_mut()
        .append_pair("actor", world)
        .append_pair("scene", scene);
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut last_observed = String::from("no getScene response yet");
    loop {
        let request = async {
            let response = session
                .client
                .get(query_url.clone())
                .header(COOKIE, &session.session_cookie)
                .send()
                .await?;
            let status = response.status();
            Ok::<_, reqwest::Error>((status, response.text().await?))
        };
        let (status, body) = match timeout_at(deadline, request).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => panic!("getScene request failed: {error}; last observed: {last_observed}"),
            Err(error) => panic!(
                "timed out waiting for the World name and admitted post: {error}; last observed: {last_observed}"
            ),
        };
        last_observed = format!("HTTP {status}: {body}");
        match status {
            StatusCode::NOT_FOUND => {}
            StatusCode::OK => {
                let page: Value = serde_json::from_str(&body)
                    .unwrap_or_else(|error| panic!("getScene returned invalid JSON: {error}; {last_observed}"));
                let posts = page["posts"]
                    .as_array()
                    .unwrap_or_else(|| panic!("getScene must return posts: {last_observed}"));
                if page["world"]["displayName"] == world_name
                    && let Some(post) = posts
                        .iter()
                        .find(|post| post["uri"] == post_uri && post["cid"] == post_cid)
                {
                    assert_eq!(
                        posts.len(),
                        1,
                        "the only post in a fresh scene is first: {last_observed}"
                    );
                    assert_eq!(page["world"]["did"], world, "{last_observed}");
                    assert_eq!(page["scene"]["uri"], scene_uri, "{last_observed}");
                    assert_eq!(page["scene"]["cid"], scene_cid, "{last_observed}");
                    assert_eq!(page["scene"]["value"]["title"], title, "{last_observed}");
                    assert_eq!(page["scene"]["value"]["openedBy"], session.did, "{last_observed}");
                    assert!(
                        page["scene"]["value"]["cast"]
                            .as_array()
                            .is_some_and(|cast| cast.iter().any(|member| member == &session.did)),
                        "the opener is in the public cast: {last_observed}"
                    );
                    let admission_prefix = format!("at://{world}/at.motet.pilot.admission/");
                    assert!(
                        post["admission"]["uri"]
                            .as_str()
                            .and_then(|uri| uri.strip_prefix(&admission_prefix))
                            .is_some_and(|key| !key.is_empty()),
                        "the admission belongs to this World: {last_observed}"
                    );
                    assert!(
                        post["admission"]["cid"].as_str().is_some_and(|cid| !cid.is_empty()),
                        "the admission has an exact version: {last_observed}"
                    );
                    assert_eq!(post["author"], session.did, "{last_observed}");
                    assert!(post["place"]["epoch"].as_u64().is_some(), "{last_observed}");
                    assert!(post["place"]["seq"].as_u64().is_some(), "{last_observed}");
                    if published["status"] == "admitted" {
                        assert_eq!(post["place"]["epoch"], published["epoch"], "{last_observed}");
                        assert_eq!(post["place"]["seq"], published["seq"], "{last_observed}");
                    }
                    assert_eq!(post["value"], record["value"], "{last_observed}");
                    return;
                }
            }
            _ => panic!("getScene failed while awaiting projection: {last_observed}"),
        }
        sleep_until((Instant::now() + Duration::from_secs(1)).min(deadline)).await;
    }
}
