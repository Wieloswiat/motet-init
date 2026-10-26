//! Script-free OAuth entry points. The host declares the raw routes in api.
//!
//! Consent and delegated-login ordering follow atproto-pds oauth/authorize.rs
//! and oauth/delegation/flow.rs at 09a868cd (MIT). Browser cookies bind the
//! external sign-in to the same user agent; every form has a separate CSRF token.

use std::fmt::Write as _;
use std::sync::Arc;

use atproto_oauth::scopes::Scope;
use axum::Json;
use axum::extract::{Form, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use motet_rules::Did;
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use super::protocol::{self, unique_header};
use super::state::{self, Entry, digest};
use super::{Error, Server, grants};
use crate::store::keys;

pub use super::protocol::authenticate;

pub(crate) async fn authorization_metadata(State(server): State<Arc<Server>>) -> Json<Value> {
    Json(json!({
        "issuer":server.issuer,
        "authorization_endpoint":format!("{}/oauth/authorize",server.issuer),
        "pushed_authorization_request_endpoint":format!("{}/oauth/par",server.issuer),
        "token_endpoint":format!("{}/oauth/token",server.issuer),
        "jwks_uri":format!("{}/oauth/jwks.json",server.issuer),
        "response_types_supported":["code"],
        "grant_types_supported":["authorization_code","refresh_token"],
        "scopes_supported":["atproto","transition:generic"],
        "code_challenge_methods_supported":["S256"],
        "token_endpoint_auth_methods_supported":["none","private_key_jwt"],
        "token_endpoint_auth_signing_alg_values_supported":["ES256","ES256K"],
        "dpop_signing_alg_values_supported":["ES256","ES256K"],
        "require_pushed_authorization_requests":true,
        "require_dpop_bound_access_tokens":true,
        "authorization_response_iss_parameter_supported":true,
        "client_id_metadata_document_supported":true,
        "request_parameter_supported":false,
        "prompt_values_supported":["login","consent"]
    }))
}

pub(crate) async fn protected_metadata(State(server): State<Arc<Server>>) -> Json<Value> {
    Json(
        json!({"resource":server.issuer,"authorization_servers":[server.issuer],"scopes_supported":["atproto"],"bearer_methods_supported":["header"]}),
    )
}

pub(crate) async fn client_metadata(State(server): State<Arc<Server>>) -> Json<Value> {
    Json(json!({
        "client_id":format!("{}/oauth/client-metadata.json",server.issuer),
        "client_name":"Motet World sign-in", "client_uri":server.issuer,
        "application_type":"web",
        "redirect_uris":[format!("{}/oauth/callback",server.issuer)],
        "grant_types":["authorization_code"], "response_types":["code"], "scope":"atproto",
        "token_endpoint_auth_method":"private_key_jwt", "token_endpoint_auth_signing_alg":"ES256",
        "dpop_bound_access_tokens":true,"jwks_uri":format!("{}/oauth/jwks.json",server.issuer)
    }))
}

pub(crate) async fn jwks(State(server): State<Arc<Server>>) -> Result<Json<Value>, Error> {
    let mut signer = atproto_oauth::jwk::generate(&server.signer).map_err(|_| Error::Internal)?;
    signer.kid = Some("as".into());
    let client = atproto_oauth::jwk::generate(&server.client_key).map_err(|_| Error::Internal)?;
    Ok(Json(json!({"keys":[signer,client]})))
}

#[derive(Deserialize)]
pub(crate) struct ParInput {
    client_id: String,
    response_type: String,
    redirect_uri: String,
    scope: String,
    state: Option<String>,
    code_challenge: String,
    code_challenge_method: String,
    dpop_jkt: Option<String>,
    login_hint: String,
    client_assertion_type: Option<String>,
    client_assertion: Option<String>,
    request: Option<String>,
    prompt: Option<String>,
    response_mode: Option<String>,
}

fn requested_scope(input: &ParInput) -> Result<Vec<Scope>, Error> {
    if input.response_type != "code" {
        return Err(Error::invalid(
            "unsupported_response_type",
            "response_type must be code",
        ));
    }
    if input.request.is_some() || input.response_mode.as_deref().is_some_and(|mode| mode != "query") {
        return Err(Error::invalid(
            "invalid_request",
            "signed request objects and non-query response modes are not supported",
        ));
    }
    if input
        .prompt
        .as_deref()
        .is_some_and(|prompt| prompt.split_whitespace().any(|p| !matches!(p, "login" | "consent")))
    {
        return Err(Error::invalid(
            "login_required",
            "interactive GM authentication and consent are required",
        ));
    }
    if input.code_challenge_method != "S256"
        || input.code_challenge.len() != 43
        || !input
            .code_challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(Error::invalid(
            "invalid_request",
            "PKCE requires a base64url SHA-256 challenge",
        ));
    }
    if !input.scope.split_whitespace().any(|scope| scope == "atproto") {
        return Err(Error::invalid("invalid_scope", "atproto scope is required"));
    }
    Scope::parse_multiple(&input.scope).map_err(|_| Error::invalid("invalid_scope", "scope syntax is invalid"))
}

pub(crate) async fn par(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    Form(input): Form<ParInput>,
) -> Result<Response, Error> {
    let requested = requested_scope(&input)?;
    let metadata = protocol::metadata(&server.fetcher, &input.client_id).await?;
    let ceiling = Scope::parse_multiple(&metadata.scope)
        .map_err(|_| Error::invalid("invalid_client_metadata", "client scope syntax is invalid"))?;
    if requested
        .iter()
        .any(|scope| !ceiling.iter().any(|grant| grant.grants(scope)))
    {
        return Err(Error::invalid(
            "invalid_scope",
            "request exceeds the client's declared scope",
        ));
    }
    protocol::redirect(&input.client_id, &input.redirect_uri)?;
    if !metadata.redirect_uris.contains(&input.redirect_uri) {
        return Err(Error::invalid("invalid_request", "redirect_uri is not registered"));
    }
    let proof = if unique_header(&headers, "dpop")?.is_some() {
        Some(server.proof(&headers, "POST", "/oauth/par", None).await?)
    } else {
        None
    };
    let jkt = proof
        .as_ref()
        .map(|proof| proof.jkt.as_str())
        .or(input.dpop_jkt.as_deref());
    if let Some(jkt) = jkt
        && (jkt.len() != 43
            || !jkt.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            || input.dpop_jkt.as_deref().is_some_and(|declared| declared != jkt))
    {
        return Err(Error::invalid(
            "invalid_dpop_proof",
            "dpop_jkt does not match the signed proof",
        ));
    }
    let dpop_jkt = jkt.map(str::to_owned);
    let client = protocol::client_auth(
        &server,
        &metadata,
        input.client_assertion_type.as_deref(),
        input.client_assertion.as_deref(),
        &format!("{}/oauth/par", server.issuer),
    )
    .await?;
    let world = if input.login_hint.starts_with("did:") {
        Did::new(input.login_hint)
    } else {
        crate::identity::resolve_handle(server.shard.reader(), &input.login_hint)
            .await
            .map_err(|_| Error::invalid("invalid_request", "login_hint must name a hosted World"))?
    };
    if crate::world::get(server.shard.reader(), &world).await?.is_none() {
        return Err(Error::invalid("invalid_request", "login_hint must name a hosted World"));
    }
    let request = state::Request {
        world,
        client_id: input.client_id,
        client_name: metadata.client_name.unwrap_or_else(|| metadata.client_id.clone()),
        client_key: client.key.clone(),
        redirect_uri: input.redirect_uri,
        scope: input.scope.split_whitespace().map(str::to_owned).collect(),
        state: input.state,
        challenge: input.code_challenge,
        dpop_jkt,
    };
    let request_uri = server.shard.run(state::Push { request, proof, client }).await??;
    Ok(no_store(
        (
            StatusCode::CREATED,
            Json(json!({"request_uri":request_uri,"expires_in":state::PAR_TTL})),
        )
            .into_response(),
    ))
}

#[derive(Deserialize)]
pub(crate) struct AuthorizeQuery {
    client_id: String,
    request_uri: String,
}

pub(crate) async fn authorize(
    State(server): State<Arc<Server>>,
    Query(query): Query<AuthorizeQuery>,
) -> Result<Response, Error> {
    let binding = state::random()?;
    let csrf = state::random()?;
    let flow = state::random()?;
    let request = server
        .shard
        .run(state::Begin {
            request_uri: query.request_uri,
            client_id: query.client_id,
            binding: binding.clone(),
            csrf: csrf.clone(),
            flow: flow.clone(),
        })
        .await??;
    let body = format!(
        "<h1>Sign in to act as a World</h1><p>World: {}</p><p>Application: {}</p><form method=\"post\" action=\"/oauth/login\"><input type=\"hidden\" name=\"flow\" value=\"{}\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><label>Your own handle or DID <input name=\"identifier\" required autocomplete=\"username\"></label><button type=\"submit\">Continue to your PDS</button></form>",
        escape(request.world.as_str()),
        escape(&request.client_name),
        escape(&flow),
        escape(&csrf)
    );
    let response = page(&server, &body, None)?;
    cookie(&server, response, &binding, state::LOGIN_TTL)
}

#[derive(Deserialize)]
pub(crate) struct LoginInput {
    flow: String,
    csrf: String,
    identifier: String,
}

pub(crate) async fn login(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    Form(input): Form<LoginInput>,
) -> Result<Response, Error> {
    let binding = binding(&headers)?;
    let entry: Entry<state::Authorization> = state::read(
        server.shard.reader(),
        &keys::oauth("authorization", &digest(&input.flow)),
        false,
    )
    .await?;
    state::browser(&entry.value, &binding, &input.csrf)?;
    let started = server.client.start(&input.identifier).await?;
    server
        .shard
        .run(state::Park {
            flow: input.flow,
            binding,
            csrf: input.csrf,
            state: started.state,
            pending: started.pending,
        })
        .await??;
    Ok(no_store(Redirect::to(&started.location).into_response()))
}

#[derive(Deserialize)]
pub(crate) struct Callback {
    state: String,
    code: String,
    iss: String,
}

pub(crate) async fn callback(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    Query(input): Query<Callback>,
) -> Result<Response, Error> {
    let binding = binding(&headers)?;
    let parked = server
        .shard
        .run(state::CompleteLogin {
            state: input.state.clone(),
            binding,
        })
        .await??;
    let actor = server
        .client
        .finish(parked.pending, &input.code, &input.state, &input.iss)
        .await?;
    let request = &parked.authorization.request;
    let scopes = grants::granted(server.shard.reader(), &request.world, &actor, &request.scope)
        .await
        .map_err(|error| match error {
            crate::spaces::SpaceError::NotAuthorized => {
                Error::invalid("access_denied", "your current roles do not allow a World session")
            }
            other => Error::Space(other),
        })?;
    let csrf = state::random()?;
    let flow = state::random()?;
    let mut body = format!(
        "<h1>Allow this application to act as the World?</h1><p>World: {}</p><p>Your account: {}</p><p>Application: {}</p><p>Granted scopes:</p><ul>",
        escape(request.world.as_str()),
        escape(actor.as_str()),
        escape(&request.client_name)
    );
    for scope in &scopes {
        write!(&mut body, "<li>{}</li>", escape(scope)).map_err(|_| Error::Internal)?;
    }
    write!(&mut body, "</ul><form method=\"post\" action=\"/oauth/consent\"><input type=\"hidden\" name=\"flow\" value=\"{}\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><button name=\"decision\" value=\"approve\">Allow</button><button name=\"decision\" value=\"deny\">Deny</button></form>", escape(&flow), escape(&csrf)).map_err(|_| Error::Internal)?;
    let response = page(&server, &body, Some(&request.redirect_uri))?;
    let mut authorization = parked.authorization;
    authorization.csrf = digest(&csrf);
    server
        .shard
        .run(state::OfferConsent {
            flow,
            consent: state::Consent {
                authorization,
                actor,
                scopes,
            },
        })
        .await??;
    Ok(response)
}

#[derive(Deserialize)]
pub(crate) struct ConsentInput {
    flow: String,
    csrf: String,
    decision: String,
}

pub(crate) async fn consent(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    Form(input): Form<ConsentInput>,
) -> Result<Response, Error> {
    let decision = match input.decision.as_str() {
        "approve" => state::Decision::Approve,
        "deny" => state::Decision::Deny,
        _ => return Err(Error::invalid("invalid_request", "consent must be approved or denied")),
    };
    let (request, code) = server
        .shard
        .run(state::Approve {
            flow: input.flow,
            csrf: input.csrf,
            binding: binding(&headers)?,
            decision,
        })
        .await??;
    let mut redirect = Url::parse(&request.redirect_uri).map_err(|_| Error::Internal)?;
    {
        let mut query = redirect.query_pairs_mut();
        match code {
            Some(code) => {
                query.append_pair("code", &code);
            }
            None => {
                query.append_pair("error", "access_denied");
            }
        }
        if let Some(state) = request.state {
            query.append_pair("state", &state);
        }
        query.append_pair("iss", &server.issuer);
    }
    cookie(
        &server,
        no_store(Redirect::to(redirect.as_str()).into_response()),
        "",
        0,
    )
}

pub(crate) async fn token(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    Form(input): Form<state::TokenInput>,
) -> Result<Response, Error> {
    let proof = server.proof(&headers, "POST", "/oauth/token", None).await?;
    let metadata = protocol::metadata(&server.fetcher, &input.client_id).await?;
    if input.grant_type == "refresh_token" && !metadata.grant_types.iter().any(|grant| grant == "refresh_token") {
        return Err(Error::invalid(
            "unauthorized_client",
            "client did not register refresh_token grant",
        ));
    }
    let client = protocol::client_auth(
        &server,
        &metadata,
        input.client_assertion_type.as_deref(),
        input.client_assertion.as_deref(),
        &format!("{}/oauth/token", server.issuer),
    )
    .await?;
    let token = server
        .shard
        .run(state::Exchange {
            input,
            client,
            proof,
            issuer: server.issuer.clone(),
            signer: Arc::clone(&server.signer),
        })
        .await??;
    match token {
        state::ExchangeOutput::Issued(token) => Ok(no_store(Json(token).into_response())),
        state::ExchangeOutput::Reused(revoke) => {
            // Commit the family revocation before returning the protocol error.
            server.shard.run(revoke).await??;
            Err(Error::invalid("invalid_grant", "refresh token has already been used"))
        }
    }
}

fn binding(headers: &HeaderMap) -> Result<String, Error> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let value = value
            .to_str()
            .map_err(|_| Error::invalid("access_denied", "invalid browser cookie"))?;
        for (name, value) in value.split(';').filter_map(|part| part.trim().split_once('=')) {
            if name == "motet_oauth" && found.replace(value.to_owned()).is_some() {
                return Err(Error::invalid("access_denied", "duplicate browser cookie"));
            }
        }
    }
    found
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::invalid("access_denied", "browser binding cookie is required"))
}

fn cookie(server: &Server, mut response: Response, value: &str, age: u64) -> Result<Response, Error> {
    let secure = if server.issuer.starts_with("https:") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!("motet_oauth={value}; Path=/oauth; Max-Age={age}; HttpOnly; SameSite=Lax{secure}");
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| Error::Internal)?,
    );
    Ok(response)
}

fn page(server: &Server, body: &str, redirect: Option<&str>) -> Result<Response, Error> {
    let html = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Motet World authorization</title><body>{body}</body></html>"
    );
    let mut response = no_store(Html(html).into_response());
    let mut form = String::from("'self' https:");
    if server.fetcher.allows_private_network() {
        form.push_str(" http:");
    }
    if let Some(redirect) = redirect {
        let url = Url::parse(redirect).map_err(|_| Error::Internal)?;
        write!(form, " {}:", url.scheme()).map_err(|_| Error::Internal)?;
    }
    let csp = format!("default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action {form}");
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&csp).map_err(|_| Error::Internal)?,
    );
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    Ok(response)
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

pub(crate) async fn cors_get(request: Request, next: Next) -> Response {
    cors(request, next, "GET").await
}

pub(crate) async fn cors_post(request: Request, next: Next) -> Response {
    cors(request, next, "POST").await
}

async fn cors(request: Request, next: Next, method: &'static str) -> Response {
    let mut response = if request.method() == axum::http::Method::OPTIONS {
        match preflight(request.headers(), method) {
            Ok(response) => response,
            Err(error) => error.into_response(),
        }
    } else {
        next.run(request).await
    };
    motet_xrpc::server::public_cors(&mut response);
    response
}

fn preflight(headers: &HeaderMap, method: &'static str) -> Result<Response, Error> {
    let origin = unique_header(headers, header::ORIGIN.as_str())?;
    let requested = unique_header(headers, header::ACCESS_CONTROL_REQUEST_METHOD.as_str())?;
    if origin.is_none_or(str::is_empty) || requested != Some(method) {
        return Err(Error::invalid("invalid_request", "invalid OAuth preflight method"));
    }
    if let Some(requested) = unique_header(headers, header::ACCESS_CONTROL_REQUEST_HEADERS.as_str())?
        && requested.split(',').map(str::trim).any(|name| {
            !["content-type", "dpop", "accept"]
                .iter()
                .any(|allowed| name.eq_ignore_ascii_case(allowed))
        })
    {
        return Err(Error::invalid("invalid_request", "unsupported OAuth request header"));
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static(method));
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, DPoP, Accept"),
    );
    Ok(response)
}
