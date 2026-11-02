//! The configured HTTPS PDS and AppView must complete an actual OAuth signup.
//!
//! This test needs `devenv up`, the development CA and DNS from `tls-setup`,
//! and the `MOTET_APPVIEW_URL`, `MOTET_PDS_RS_URL` and `DATABASE_URL` exported
//! by the dev shell. The PDS must allow signup without an invite or email
//! verification, with its hostname as the account handle suffix.

mod oauth_support;

use oauth_support::OAuthSession;
use reqwest::StatusCode;
use reqwest::header::COOKIE;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use sqlx::postgres::PgPoolOptions;
use url::Url;

// An in-memory-only session must not pass as durable service state.
async fn assert_persisted_session(database_url: &str, session_cookie: &str) {
    let session_secret = session_cookie
        .split_once('=')
        .expect("the AppView session has a value")
        .1;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .expect("connect to the AppView database");
    let id = Sha256::digest(session_secret.as_bytes());
    let persisted: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM service.oauth_sessions WHERE id = $1 AND NOT revoked AND expires_at > now())",
    )
    .bind(id.as_slice())
    .fetch_one(&pool)
    .await
    .expect("read the persisted AppView session");
    assert!(persisted, "the browser session must be committed to PostgreSQL");
}

async fn assert_session_identity(get_session: &Url, signed_in: &OAuthSession) {
    let session = signed_in
        .client
        .get(get_session.clone())
        .header(COOKIE, &signed_in.session_cookie)
        .send()
        .await
        .expect("read the AppView session on a separate request");
    assert_eq!(session.status(), StatusCode::OK, "the browser session authenticates");
    let session: Value = session.json().await.expect("getSession returns JSON");
    assert_eq!(session["did"].as_str(), Some(signed_in.did.as_str()));
    assert_eq!(session["handle"].as_str(), Some(signed_in.handle.as_str()));
}

// A signup must authenticate the new PDS identity and persist its session.
// Replacing it with another account must revoke the previous cookie; otherwise
// a signed-out account remains usable after the browser switches accounts.
#[tokio::test]
#[ignore = "integration: HTTPS PDS, AppView, PLC, PostgreSQL and local DNS"]
async fn pds_signup_and_consent_persist_the_new_identity_in_the_appview() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must name the running AppView database");
    let first = oauth_support::create_session(None).await;
    let get_session = first
        .app
        .join("/xrpc/at.motet.pilot.auth.getSession")
        .expect("the AppView session URL is valid");

    let anonymous = first
        .client
        .get(get_session.clone())
        .send()
        .await
        .expect("request anonymous session");
    assert_eq!(
        anonymous.status(),
        StatusCode::UNAUTHORIZED,
        "an anonymous browser has no AppView session"
    );
    assert_persisted_session(&database_url, &first.session_cookie).await;
    assert_session_identity(&get_session, &first).await;

    let second = oauth_support::create_session(Some(&first)).await;
    assert_eq!(
        first.app.origin(),
        second.app.origin(),
        "the browser must remain on the AppView origin"
    );
    assert_ne!(first.did, second.did, "the two signups must create distinct accounts");

    let old_session = first
        .client
        .get(get_session.clone())
        .header(COOKIE, &first.session_cookie)
        .send()
        .await
        .expect("check the replaced browser session");
    assert_eq!(
        old_session.status(),
        StatusCode::UNAUTHORIZED,
        "the old account must not remain authenticated after replacement"
    );
    assert_persisted_session(&database_url, &second.session_cookie).await;
    assert_session_identity(&get_session, &second).await;
}
