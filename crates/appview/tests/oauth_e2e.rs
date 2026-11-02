//! The configured HTTPS PDS and AppView must complete an actual OAuth signup.
//!
//! This test needs `devenv up`, the development CA and DNS from `tls-setup`,
//! and the `MOTET_APPVIEW_URL`, `MOTET_PDS_RS_URL` and `DATABASE_URL` exported
//! by the dev shell. The PDS must allow signup without an invite or email
//! verification, with its hostname as the account handle suffix.

use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, COOKIE, LOCATION, ORIGIN, SET_COOKIE};
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use sqlx::postgres::PgPoolOptions;
use url::Url;
use uuid::Uuid;

fn origin_from_env(name: &str) -> Url {
    let value = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name the running development service"));
    let url = Url::parse(&value).expect("the configured service origin is an absolute URL");
    assert_eq!(url.scheme(), "https", "{name} must use HTTPS");
    assert!(url.has_host() && url.path() == "/" && url.query().is_none() && url.fragment().is_none());
    url
}

fn expect_status(response: &Response, status: StatusCode, stage: &str) {
    assert_eq!(response.status(), status, "{stage}");
}

fn redirect(response: &Response, status: StatusCode, stage: &str) -> Url {
    expect_status(response, status, stage);
    let location = response
        .headers()
        .get(LOCATION)
        .expect("a redirect includes Location")
        .to_str()
        .expect("Location is valid ASCII");
    response
        .url()
        .join(location)
        .expect("the redirect target is a valid URL")
}

fn cookie(response: &Response, prefix: &str, host_only: bool) -> String {
    let value = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .find(|header| header.starts_with(prefix))
        .expect("the response sets the expected browser cookie");
    let pair = value.split(';').next().expect("the cookie has a name and value");
    let (_, secret) = pair.split_once('=').expect("the cookie has a value");
    assert!(!secret.is_empty(), "the browser cookie must not be empty");
    if host_only {
        for attribute in ["Path=/", "HttpOnly", "Secure", "SameSite=Lax"] {
            assert!(
                value
                    .split(';')
                    .map(str::trim)
                    .any(|part| part.eq_ignore_ascii_case(attribute)),
                "the AppView cookie must have {attribute}"
            );
        }
        assert!(
            !value
                .split(';')
                .map(str::trim)
                .any(|part| part.to_ascii_lowercase().starts_with("domain=")),
            "the AppView cookie must be host-only"
        );
    }
    pair.to_owned()
}

fn query_value(url: &Url, name: &str) -> String {
    url.query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| panic!("the redirect must include {name}"))
}

// These forms are the pinned atproto-crates PDS's script-free portal UI. Read
// the values the user would submit, rather than constructing OAuth state here.
fn form<'a>(page: &'a str, action: &str) -> &'a str {
    let opening = format!(r#"<form method="POST" action="{action}">"#);
    page.split_once(&opening)
        .and_then(|(_, rest)| rest.split_once("</form>"))
        .map(|(form, _)| form)
        .expect("the PDS page contains the expected form")
}

fn hidden_field(form: &str, name: &str) -> String {
    let marker = format!(r#"name="{name}" value=""#);
    let encoded = form
        .split_once(&marker)
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(value, _)| value)
        .unwrap_or_else(|| panic!("the PDS form must contain {name}"));
    encoded
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn form_body(fields: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in fields {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn navigation(request: reqwest::RequestBuilder, site: &str) -> reqwest::RequestBuilder {
    request
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Site", site)
}

async fn create_account(client: &Client, pds: &Url, signup: Url, request_uri: &str) -> (Url, String, String) {
    let signup_page = navigation(client.get(signup.clone()), "same-origin")
        .send()
        .await
        .expect("open the PDS signup form");
    expect_status(&signup_page, StatusCode::OK, "PDS signup form");
    let signup_html = signup_page.text().await.expect("read the signup form");
    let signup_form = form(&signup_html, "/account/signup");
    for name in ["name", "email", "password"] {
        assert!(
            signup_form.contains(&format!(r#"name="{name}""#)),
            "signup asks for {name}"
        );
    }
    let next = hidden_field(signup_form, "next");
    assert_eq!(
        next,
        query_value(&signup, "next"),
        "the form must preserve the authorization continuation"
    );

    let username = format!("oauth{}", Uuid::now_v7().simple());
    let handle = format!("{username}.{}", pds.host_str().expect("the PDS has a hostname"));
    let email = format!("{username}@example.test");
    let password = format!("motet-oauth-test-{}", Uuid::now_v7().simple());
    let submitted = client
        .post(pds.join("/account/signup").expect("the PDS signup action is valid"))
        .header(ORIGIN, pds.origin().ascii_serialization())
        .header("Sec-Fetch-Site", "same-origin")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form_body(&[
            ("next", &next),
            ("name", &username),
            ("email", &email),
            ("password", &password),
        ]))
        .send()
        .await
        .expect("create the PDS account through its signup form");
    let consent_url = redirect(&submitted, StatusCode::SEE_OTHER, "PDS creates the account");
    assert_eq!(consent_url.origin(), pds.origin());
    assert_eq!(consent_url.path(), "/oauth/authorize");
    assert_eq!(query_value(&consent_url, "request_uri"), request_uri);
    let pds_cookie = cookie(&submitted, "atproto_pds_portal=", false);
    (consent_url, pds_cookie, handle)
}

async fn approve(client: &Client, app: &Url, pds: &Url, consent_url: Url, request_uri: &str, pds_cookie: &str) -> Url {
    let consent = navigation(client.get(consent_url).header(COOKIE, pds_cookie), "same-origin")
        .send()
        .await
        .expect("open consent for the new PDS account");
    expect_status(&consent, StatusCode::OK, "PDS consent page");
    let consent_html = consent.text().await.expect("read the PDS consent form");
    let consent_form = form(&consent_html, "/oauth/authorize");
    assert!(
        consent_form.contains(r#"name="approve" value="true""#),
        "the PDS asks for consent"
    );
    let consent_request = hidden_field(consent_form, "request_uri");
    let csrf = hidden_field(consent_form, "csrf");
    assert_eq!(consent_request, request_uri);
    assert!(!csrf.is_empty(), "consent is bound to the PDS browser session");

    let approved = client
        .post(pds.join("/oauth/authorize").expect("the PDS consent action is valid"))
        .header(ORIGIN, pds.origin().ascii_serialization())
        .header("Sec-Fetch-Site", "same-origin")
        .header(COOKIE, pds_cookie)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form_body(&[
            ("request_uri", &consent_request),
            ("csrf", &csrf),
            ("approve", "true"),
        ]))
        .send()
        .await
        .expect("approve the PDS consent form");
    let callback = redirect(&approved, StatusCode::SEE_OTHER, "PDS returns an authorization code");
    assert_eq!(callback.origin(), app.origin());
    assert_eq!(callback.path(), "/oauth/callback");
    assert!(!query_value(&callback, "code").is_empty());
    assert!(!query_value(&callback, "state").is_empty());
    assert_eq!(query_value(&callback, "iss"), pds.origin().ascii_serialization());
    callback
}

// A newly created PDS account must become the same authenticated AppView
// identity. A lost PAR continuation, wrong OAuth issuer, cookie binding, or
// unpersisted grant must fail rather than looking like a successful signup.
#[tokio::test]
#[ignore = "integration: HTTPS PDS, AppView, PLC, PostgreSQL and local DNS"]
async fn pds_signup_and_consent_persist_the_new_identity_in_the_appview() {
    let app = origin_from_env("MOTET_APPVIEW_URL");
    let pds = origin_from_env("MOTET_PDS_RS_URL");
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must name the running AppView database");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("the HTTPS browser client can be built");
    let get_session = app
        .join("/xrpc/at.motet.pilot.auth.getSession")
        .expect("the AppView session URL is valid");

    let anonymous = client
        .get(get_session.clone())
        .send()
        .await
        .expect("request anonymous session");
    expect_status(
        &anonymous,
        StatusCode::UNAUTHORIZED,
        "an anonymous browser has no AppView session",
    );

    let start = client
        .get(app.join("/oauth/sign-up").expect("the signup URL is valid"))
        .send()
        .await
        .expect("start AppView signup");
    let authorize = redirect(&start, StatusCode::TEMPORARY_REDIRECT, "AppView starts PAR");
    assert_eq!(
        authorize.origin(),
        pds.origin(),
        "PAR must redirect to the configured PDS"
    );
    assert_eq!(authorize.path(), "/oauth/authorize");
    assert_eq!(
        query_value(&authorize, "client_id"),
        app.join("/oauth/client-metadata.json")
            .expect("the client metadata URL is valid")
            .as_str()
    );
    let request_uri = query_value(&authorize, "request_uri");
    assert!(!request_uri.is_empty());
    let login_cookie = cookie(&start, "__Host-motet_login_", true);

    let authorization = navigation(client.get(authorize), "same-site")
        .send()
        .await
        .expect("open the PDS authorization page");
    let signup = redirect(&authorization, StatusCode::SEE_OTHER, "prompt=create opens PDS signup");
    assert_eq!(signup.origin(), pds.origin());
    assert_eq!(
        signup.path(),
        "/account/signup",
        "signup must begin at the PDS, not a local account API"
    );

    let (consent_url, pds_cookie, handle) = create_account(&client, &pds, signup, &request_uri).await;

    // Use the PDS's account directory as the independent identity oracle.
    let mut resolve = pds
        .join("/xrpc/com.atproto.identity.resolveHandle")
        .expect("the PDS handle resolution URL is valid");
    resolve.query_pairs_mut().append_pair("handle", &handle);
    let resolved = client
        .get(resolve)
        .send()
        .await
        .expect("resolve the newly created account");
    expect_status(&resolved, StatusCode::OK, "PDS resolves the new handle");
    let resolved: Value = resolved.json().await.expect("PDS handle resolution returns JSON");
    let did = resolved["did"].as_str().expect("the PDS account has a DID");
    assert!(did.starts_with("did:plc:"), "the local PDS creates a PLC identity");

    let callback = approve(&client, &app, &pds, consent_url, &request_uri, &pds_cookie).await;

    let completed = navigation(client.get(callback).header(COOKIE, &login_cookie), "same-site")
        .send()
        .await
        .expect("complete the AppView callback");
    let home = redirect(&completed, StatusCode::SEE_OTHER, "AppView completes signup");
    assert_eq!(home.origin(), app.origin());
    assert_eq!(home.path(), "/");
    let session_cookie = cookie(&completed, "__Host-motet_session=", true);
    let session_secret = session_cookie
        .split_once('=')
        .expect("the AppView session has a value")
        .1;

    // Check the running AppView's database before using the cookie in a new
    // request; an in-memory-only session must not pass as durable service state.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
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

    let session = client
        .get(get_session)
        .header(COOKIE, &session_cookie)
        .send()
        .await
        .expect("read the AppView session on a separate request");
    expect_status(&session, StatusCode::OK, "the new browser session authenticates");
    let session: Value = session.json().await.expect("getSession returns JSON");
    assert_eq!(session["did"].as_str(), Some(did));
    assert_eq!(session["handle"].as_str(), Some(handle.as_str()));
}
