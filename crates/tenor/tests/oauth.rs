//! World sessions cross real HTTP, an independent GM authorization server and the shard.
//! The fake PDS verifies the tenor's confidential-client, PKCE and `DPoP` credentials;
//! assertions below concern the tenor's authority decisions, not the fake's responses.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use atproto_identity::key::{KeyData, KeyType, generate_key, sign, to_public};
use atproto_identity::model::Document;
use atproto_oauth::dpop::{self, DpopValidationConfig};
use atproto_oauth::jwk::{self, WrappedJsonWebKeySet};
use atproto_oauth::jwt::{self, Claims, Header, JoseClaims};
use atproto_oauth::pkce;
use atproto_oauth::reqwest;
use atproto_oauth::workflow::generate_state;
use atproto_record::{datetime::Datetime, tid::Tid};
use atproto_space::space_repo::OpAction;
use atproto_space::storage::SpaceRepoStorage as _;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_tenor::{api, pds, repo};
use object_store::memory::InMemory;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use url::Url;

const WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";
const WRITER: &str = "did:plc:cccccccccccccccccccccccc";
const OUTSIDER: &str = "did:plc:dddddddddddddddddddddddd";
const CHARACTER: &str = "at.motet.pilot.character";
const PROFILE: &str = "at.motet.pilot.profile";
const CREATE_SCOPE: &str = "repo:at.motet.pilot.character?action=create";
const CREATE: &str = "com.atproto.repo.createRecord";
const DELETE: &str = "com.atproto.repo.deleteRecord";
const ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";
type Fields = BTreeMap<String, String>;

fn fields<const N: usize>(pairs: [(&str, &str); N]) -> Fields {
    pairs
        .into_iter()
        .map(|(name, value)| (name.into(), value.into()))
        .collect()
}

fn field<'a>(form: &'a Fields, name: &str) -> anyhow::Result<&'a str> {
    form.get(name)
        .map(String::as_str)
        .with_context(|| format!("missing {name}"))
}

fn jwt_part<T: DeserializeOwned>(token: &str, part: usize) -> anyhow::Result<T> {
    let encoded = token.split('.').nth(part).context("missing JWT segment")?;
    Ok(serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded)?)?)
}

fn json_str<'a>(value: &'a Value, name: &str) -> &'a str {
    value
        .get(name)
        .and_then(Value::as_str)
        .expect("fixture JSON contains the required string field")
}

fn fractional_proof(key: &KeyData, proof: &str) -> String {
    let header = proof.split('.').next().expect("minted proof has a JWT header");
    let mut claims: Value = jwt_part(proof, 1).expect("minted proof has JSON claims");
    for name in ["iat", "exp"] {
        let claim = claims.get_mut(name).expect("minted proof includes its time claims");
        let seconds = claim.as_u64().expect("upstream proof initially uses whole seconds");
        *claim = serde_json::from_str(&format!("{seconds}.25")).expect("fractional seconds are valid JSON numbers");
    }
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("JWT claims serialize as JSON"));
    let input = format!("{header}.{payload}");
    let signature = sign(key, input.as_bytes()).expect("fixture private key signs the JWT");
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
}

fn header_allows(headers: &HeaderMap, name: &str, value: &str) -> bool {
    headers.get_all(name).iter().any(|header| {
        header
            .to_str()
            .expect("CORS header contains text")
            .split(',')
            .any(|part| {
                let part = part.trim();
                part == "*" || part.eq_ignore_ascii_case(value)
            })
    })
}

fn client_assertion(key: &KeyData, client_id: &str, audience: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock follows the Unix epoch")
        .as_secs();
    let header = Header::try_from(to_public(key).expect("fixture private key has a public component"))
        .expect("fixture EC key has a JOSE algorithm");
    jwt::mint(
        key,
        &header,
        &Claims::new(JoseClaims {
            issuer: Some(client_id.into()),
            subject: Some(client_id.into()),
            audience: Some(audience.into()),
            issued_at: Some(now),
            expiration: Some(now + 60),
            json_web_token_id: Some(generate_state()),
            ..JoseClaims::default()
        }),
    )
    .expect("fixture signing key can mint the client assertion")
}

fn fractional_assertion(key: &KeyData, client_id: &str, audiences: &[&str]) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock follows the Unix epoch")
        .as_secs();
    let header = Header::try_from(to_public(key).expect("fixture private key has a public component"))
        .expect("fixture EC key has a JOSE algorithm");
    let claims = json!({
        "iss": client_id, "sub": client_id, "aud": audiences, "jti": generate_state(),
        "iat": serde_json::from_str::<Value>(&format!("{now}.25")).expect("fractional iat is a JSON number"),
        "exp": serde_json::from_str::<Value>(&format!("{}.25", now + 60)).expect("fractional exp is a JSON number"),
    });
    let header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("JWT header serializes as JSON"));
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("JWT claims serialize as JSON"));
    let input = format!("{header}.{payload}");
    let signature = sign(key, input.as_bytes()).expect("fixture private key signs the JWT");
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
}

fn document(did: &str, endpoint: &str, key: &KeyData) -> Document {
    let public = to_public(key)
        .expect("fixture private key has a public component")
        .to_string();
    serde_json::from_value(json!({
        "id": did,
        "alsoKnownAs": [if did == WORLD { "at://world.motet.test" } else { "at://gm.motet.test" }],
        "verificationMethod": [{"id": format!("{did}#atproto"), "type": "Multikey", "controller": did,
            "publicKeyMultibase": public.strip_prefix("did:key:").expect("public key uses did:key encoding")}],
        "service": [{"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": endpoint}],
    }))
    .expect("fixture JSON has the DID document shape")
}

struct Initialize(String);

impl Op for Initialize {
    type Output = WorldInfo;
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<WorldInfo, SpaceError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("fresh fixture World");
        let signing = txn
            .generate_key(&world, Purpose::Signing)
            .expect("fixture World signing key generation succeeds");
        txn.generate_key(&world, Purpose::Kem)
            .expect("fixture World KEM key generation succeeds");
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: "world.motet.test".into(),
            epoch: 1,
            status: Status::Active,
            created: Datetime::now().into_string(),
            document: document(WORLD, &self.0, &signing),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        txn.put(keys::handle(&info.handle), WORLD.as_bytes().into());
        repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        let mut permissions = SpaceWrite::create(
            "group.opensocial.permissions",
            "self",
            &json!({"grants": [
                {"role": "owner", "assignable": ["owner", "writer"], "scopes": ["repo:*"]},
                {"role": "writer", "assignable": [], "scopes": [CREATE_SCOPE]},
            ]}),
        )?;
        permissions.action = OpAction::Update;
        spaces::write_as_world(
            txn,
            &spaces::world_space(&world, spaces::MEMBERS)?,
            vec![
                SpaceWrite::create("group.opensocial.role", "writer", &json!({"displayName": "Writer"}))?,
                SpaceWrite::create(
                    "group.opensocial.membership",
                    WRITER,
                    &json!({
                        "member": WRITER, "roles": ["writer"], "createdAt": Datetime::now().into_string(),
                    }),
                )?,
                permissions,
            ],
        )
        .await?;
        Ok(info)
    }
}

struct RemoveAuthority {
    collection: &'static str,
    rkey: &'static str,
}

impl Op for RemoveAuthority {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::write_as_world(
            txn,
            &spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)?,
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: self.collection.into(),
                rkey: self.rkey.into(),
                value: None,
                swap: None,
            }],
        )
        .await?;
        Ok(())
    }
}

struct UpstreamGrant {
    client_id: String,
    redirect_uri: String,
    state: String,
    challenge: String,
    subject: String,
    jkt: String,
}

struct FakePds {
    origin: String,
    fetcher: Fetcher,
    documents: BTreeMap<String, Document>,
    client_key: Arc<KeyData>,
    nonce: String,
    pars: Mutex<BTreeMap<String, UpstreamGrant>>,
    codes: Mutex<BTreeMap<String, UpstreamGrant>>,
    proofs: Mutex<BTreeSet<String>>,
    assertions: Mutex<BTreeSet<String>>,
    subject_override: Mutex<Option<&'static str>>,
}

enum ProofCheck {
    Verified(String),
    NonceRequired,
}

impl FakePds {
    fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/plc/{did}", get(fake_document))
            .route("/.well-known/oauth-protected-resource", get(fake_resource))
            .route("/.well-known/oauth-authorization-server", get(fake_metadata))
            .route("/client/{kind}", get(fake_client_metadata))
            .route("/par", post(fake_par))
            .route("/authorize", get(fake_authorize))
            .route("/token", post(fake_token))
            .with_state(Arc::clone(self))
    }

    fn proof(&self, headers: &HeaderMap, path: &str) -> anyhow::Result<ProofCheck> {
        let proof = headers.get("dpop").context("DPoP is required")?.to_str()?;
        let config = DpopValidationConfig::for_authorization("POST", &format!("{}{path}", self.origin));
        let jkt = dpop::validate_dpop_jwt(proof, &config)?;
        let claims: Claims = jwt_part(proof, 1)?;
        if claims.jose.nonce.as_deref() != Some(self.nonce.as_str()) {
            return Ok(ProofCheck::NonceRequired);
        }
        let jti = claims.jose.json_web_token_id.context("missing proof jti")?;
        anyhow::ensure!(
            self.proofs
                .lock()
                .expect("fixture mutex is not poisoned")
                .insert(format!("{jkt}/{jti}")),
            "replayed upstream DPoP proof"
        );
        Ok(ProofCheck::Verified(jkt))
    }

    async fn authenticate(&self, form: &Fields) -> anyhow::Result<()> {
        let client_id = field(form, "client_id")?;
        let metadata: Value = serde_json::from_slice(&self.fetcher.get_bytes(client_id).await?)?;
        anyhow::ensure!(
            metadata.get("client_id").and_then(Value::as_str) == Some(client_id),
            "client metadata ID mismatch"
        );
        anyhow::ensure!(
            metadata.get("token_endpoint_auth_method").and_then(Value::as_str) == Some("private_key_jwt"),
            "tenor must be confidential"
        );
        anyhow::ensure!(
            metadata.get("scope").and_then(Value::as_str) == Some("atproto"),
            "GM login must not request World write rights"
        );
        anyhow::ensure!(
            metadata.get("dpop_bound_access_tokens").and_then(Value::as_bool) == Some(true),
            "tenor must bind its upstream token"
        );
        anyhow::ensure!(
            field(form, "client_assertion_type")? == ASSERTION_TYPE,
            "wrong assertion type"
        );
        let assertion = field(form, "client_assertion")?;
        let header: Header = jwt_part(assertion, 0)?;
        let jwks_uri = metadata
            .get("jwks_uri")
            .and_then(Value::as_str)
            .context("missing tenor JWKS")?;
        let keyset: WrappedJsonWebKeySet = serde_json::from_slice(&self.fetcher.get_bytes(jwks_uri).await?)?;
        let public = keyset
            .keys
            .iter()
            .find(|key| key.kid == header.key_id)
            .context("unknown assertion key")?;
        let claims = jwt::verify(assertion, &jwk::to_key_data(public)?)?;
        anyhow::ensure!(
            claims.jose.issuer.as_deref() == Some(client_id),
            "assertion issuer mismatch"
        );
        anyhow::ensure!(
            claims.jose.subject.as_deref() == Some(client_id),
            "assertion subject mismatch"
        );
        anyhow::ensure!(
            claims.jose.audience.as_deref() == Some(self.origin.as_str()),
            "assertion audience mismatch"
        );
        let issued = claims.jose.issued_at.context("missing assertion iat")?;
        anyhow::ensure!(
            claims
                .jose
                .expiration
                .is_some_and(|expires| expires > issued && expires - issued <= 300),
            "assertion lifetime is not bounded"
        );
        let jti = claims.jose.json_web_token_id.context("missing assertion jti")?;
        anyhow::ensure!(
            self.assertions
                .lock()
                .expect("fixture mutex is not poisoned")
                .insert(format!("{client_id}/{jti}")),
            "assertion replay"
        );
        if let Some(redirect) = form.get("redirect_uri") {
            anyhow::ensure!(
                metadata
                    .get("redirect_uris")
                    .and_then(Value::as_array)
                    .context("missing redirects")?
                    .iter()
                    .any(|uri| uri == redirect),
                "redirect is not registered"
            );
        }
        Ok(())
    }

    async fn push(&self, form: Fields, jkt: String) -> anyhow::Result<Value> {
        self.authenticate(&form).await?;
        anyhow::ensure!(field(&form, "response_type")? == "code", "wrong response type");
        anyhow::ensure!(field(&form, "code_challenge_method")? == "S256", "PKCE must use S256");
        anyhow::ensure!(
            field(&form, "scope")? == "atproto",
            "GM consent must be authentication only"
        );
        let subject = field(&form, "login_hint")?;
        anyhow::ensure!([OWNER, WRITER, OUTSIDER].contains(&subject), "unknown GM account");
        let challenge = field(&form, "code_challenge")?;
        anyhow::ensure!(challenge.len() == 43, "invalid PKCE challenge");
        let request_uri = format!("urn:ietf:params:oauth:request_uri:{}", generate_state());
        self.pars.lock().expect("fixture mutex is not poisoned").insert(
            request_uri.clone(),
            UpstreamGrant {
                client_id: field(&form, "client_id")?.into(),
                redirect_uri: field(&form, "redirect_uri")?.into(),
                state: field(&form, "state")?.into(),
                challenge: challenge.into(),
                subject: subject.into(),
                jkt,
            },
        );
        Ok(json!({"request_uri": request_uri, "expires_in": 90}))
    }

    fn authorize(&self, form: &Fields) -> anyhow::Result<Url> {
        let grant = self
            .pars
            .lock()
            .expect("fixture mutex is not poisoned")
            .remove(field(form, "request_uri")?)
            .context("unknown or reused PAR")?;
        anyhow::ensure!(
            grant.client_id == field(form, "client_id")?,
            "PAR belongs to another client"
        );
        let code = generate_state();
        let mut callback = Url::parse(&grant.redirect_uri)?;
        callback
            .query_pairs_mut()
            .append_pair("state", &grant.state)
            .append_pair("iss", &self.origin)
            .append_pair("code", &code);
        self.codes
            .lock()
            .expect("fixture mutex is not poisoned")
            .insert(code, grant);
        Ok(callback)
    }

    async fn token(&self, form: Fields, jkt: &str) -> anyhow::Result<Value> {
        self.authenticate(&form).await?;
        anyhow::ensure!(
            field(&form, "grant_type")? == "authorization_code",
            "unexpected GM grant type"
        );
        let grant = self
            .codes
            .lock()
            .expect("fixture mutex is not poisoned")
            .remove(field(&form, "code")?)
            .context("unknown or reused GM code")?;
        anyhow::ensure!(grant.client_id == field(&form, "client_id")?, "code client mismatch");
        anyhow::ensure!(
            grant.redirect_uri == field(&form, "redirect_uri")?,
            "code redirect mismatch"
        );
        anyhow::ensure!(
            grant.challenge == pkce::challenge(field(&form, "code_verifier")?),
            "PKCE verifier mismatch"
        );
        anyhow::ensure!(grant.jkt == jkt, "DPoP key changed during GM sign-in");
        let subject = self
            .subject_override
            .lock()
            .expect("fixture mutex is not poisoned")
            .map_or(grant.subject, str::to_owned);
        Ok(
            json!({"access_token": generate_state(), "token_type": "DPoP", "scope": "atproto",
            "expires_in": 300, "sub": subject}),
        )
    }
}

fn fake_error(error: &anyhow::Error) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "invalid_request", "error_description": error.to_string()})),
    )
        .into_response()
}

fn fake_nonce(nonce: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        [("dpop-nonce", nonce)],
        Json(json!({"error": "use_dpop_nonce"})),
    )
        .into_response()
}

async fn fake_document(State(state): State<Arc<FakePds>>, Path(did): Path<String>) -> Response {
    state.documents.get(&did).map_or_else(
        || StatusCode::NOT_FOUND.into_response(),
        |doc| Json(doc).into_response(),
    )
}

async fn fake_resource(State(state): State<Arc<FakePds>>) -> Json<Value> {
    Json(
        json!({"resource": state.origin, "authorization_servers": [state.origin],
        "scopes_supported": ["atproto"], "bearer_methods_supported": ["header"]}),
    )
}

async fn fake_metadata(State(state): State<Arc<FakePds>>) -> Json<Value> {
    Json(json!({
        "issuer": state.origin, "authorization_endpoint": format!("{}/authorize", state.origin),
        "pushed_authorization_request_endpoint": format!("{}/par", state.origin),
        "token_endpoint": format!("{}/token", state.origin), "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "scopes_supported": ["atproto", "transition:generic"], "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
        "token_endpoint_auth_signing_alg_values_supported": ["ES256"], "dpop_signing_alg_values_supported": ["ES256"],
        "require_pushed_authorization_requests": true, "authorization_response_iss_parameter_supported": true,
        "client_id_metadata_document_supported": true,
    }))
}

async fn fake_client_metadata(State(state): State<Arc<FakePds>>, Path(kind): Path<String>) -> Response {
    let auth = match kind.as_str() {
        "public.json" => "none",
        "confidential.json" => "private_key_jwt",
        "secret.json" => "client_secret_post",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let mut metadata = json!({
        "client_id": format!("{}/client/{kind}", state.origin), "client_name": "World editor",
        "application_type": "web", "redirect_uris": [format!("{}/client/callback", state.origin)],
        "grant_types": ["authorization_code", "refresh_token"], "response_types": ["code"],
        "scope": "atproto repo:*", "token_endpoint_auth_method": auth, "dpop_bound_access_tokens": true,
    });
    if auth == "private_key_jwt" {
        let metadata = metadata.as_object_mut().expect("client metadata is a JSON object");
        metadata.insert("token_endpoint_auth_signing_alg".into(), "ES256".into());
        metadata.insert(
            "jwks".into(),
            json!({"keys": [
                jwk::generate(&state.client_key).expect("fixture key has a publishable JWK")
            ]}),
        );
    }
    Json(metadata).into_response()
}

async fn fake_par(State(state): State<Arc<FakePds>>, headers: HeaderMap, Form(form): Form<Fields>) -> Response {
    let jkt = match state.proof(&headers, "/par") {
        Ok(ProofCheck::Verified(jkt)) => jkt,
        Ok(ProofCheck::NonceRequired) => return fake_nonce(&state.nonce),
        Err(error) => return fake_error(&error),
    };
    state.push(form, jkt).await.map_or_else(
        |error| fake_error(&error),
        |value| (StatusCode::CREATED, Json(value)).into_response(),
    )
}

async fn fake_authorize(State(state): State<Arc<FakePds>>, Query(form): Query<Fields>) -> Response {
    state.authorize(&form).map_or_else(
        |error| fake_error(&error),
        |url| Redirect::to(url.as_str()).into_response(),
    )
}

async fn fake_token(State(state): State<Arc<FakePds>>, headers: HeaderMap, Form(form): Form<Fields>) -> Response {
    let jkt = match state.proof(&headers, "/token") {
        Ok(ProofCheck::Verified(jkt)) => jkt,
        Ok(ProofCheck::NonceRequired) => return fake_nonce(&state.nonce),
        Err(error) => return fake_error(&error),
    };
    state
        .token(form, &jkt)
        .await
        .map_or_else(|error| fake_error(&error), |value| Json(value).into_response())
}

struct Client {
    id: String,
    redirect: String,
    signing: Option<Arc<KeyData>>,
    dpop: KeyData,
}

impl Client {
    fn authenticate(&self, form: &mut Fields, issuer: &str) {
        if let Some(signing) = &self.signing {
            form.insert("client_assertion_type".into(), ASSERTION_TYPE.into());
            form.insert("client_assertion".into(), client_assertion(signing, &self.id, issuer));
        }
    }

    fn par(&self, scope: &str, challenge: &str, state: &str) -> Fields {
        fields([
            ("client_id", &self.id),
            ("response_type", "code"),
            ("redirect_uri", &self.redirect),
            ("scope", scope),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("login_hint", WORLD),
        ])
    }

    fn exchange(&self, code: &Code) -> Fields {
        fields([
            ("grant_type", "authorization_code"),
            ("client_id", &self.id),
            ("code", &code.value),
            ("redirect_uri", &self.redirect),
            ("code_verifier", &code.verifier),
        ])
    }

    fn refresh(&self, token: &Value) -> Fields {
        fields([
            ("grant_type", "refresh_token"),
            ("client_id", &self.id),
            ("refresh_token", json_str(token, "refresh_token")),
        ])
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    async fn send(request: reqwest::RequestBuilder) -> Self {
        Self::receive(request.send().await.expect("in-process server answers the request")).await
    }

    async fn receive(response: reqwest::Response) -> Self {
        Self {
            status: response.status(),
            headers: response.headers().clone(),
            body: response.text().await.expect("in-process response body is readable"),
        }
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|error| panic!("{}: {} ({error})", self.status, self.body))
    }

    fn expect(&self, status: StatusCode) {
        assert_eq!(self.status, status, "{}", self.body);
    }

    fn location(&self) -> Url {
        self.expect(StatusCode::SEE_OTHER);
        Url::parse(self.header("location")).expect("redirect Location is an absolute URL")
    }

    fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .expect("response includes the required header")
            .to_str()
            .expect("response header contains text")
    }

    fn nonce(&self, status: StatusCode) -> &str {
        self.expect(status);
        if status == StatusCode::BAD_REQUEST {
            assert_eq!(self.json().get("error").and_then(Value::as_str), Some("use_dpop_nonce"));
        } else {
            assert!(self.header("www-authenticate").contains("use_dpop_nonce"));
        }
        self.header("dpop-nonce")
    }

    fn cookie(&self) -> String {
        self.headers
            .get_all("set-cookie")
            .iter()
            .find_map(|cookie| {
                let cookie = cookie
                    .to_str()
                    .expect("Set-Cookie contains text")
                    .split(';')
                    .next()
                    .expect("Set-Cookie has a first segment");
                cookie.starts_with("motet_oauth=").then(|| cookie.to_owned())
            })
            .expect("authorization installs a browser-binding cookie")
    }

    fn public_cors(&self, origin: &str) {
        let allowed = self.header("access-control-allow-origin");
        assert!(
            allowed == "*" || allowed == origin,
            "origin {origin} cannot read the response"
        );
        assert!(!self.headers.contains_key("access-control-allow-credentials"));
    }

    fn private_cors(&self) {
        assert!(!self.headers.contains_key("access-control-allow-origin"));
        assert!(!self.headers.contains_key("access-control-allow-credentials"));
    }
}

fn hidden(html: &str, name: &str) -> String {
    let name = format!("name=\"{name}\"");
    html.split("<input")
        .filter_map(|input| input.split_once('>').map(|(tag, _)| tag))
        .find(|tag| tag.contains(&name))
        .and_then(|tag| tag.split_once("value=\""))
        .and_then(|(_, value)| value.split_once('"'))
        .map(|(value, _)| value.to_owned())
        .expect("server form has the requested hidden control")
}

struct Login {
    verifier: String,
    state: String,
    cookie: String,
    form: Fields,
}

struct Callback {
    login: Login,
    url: Url,
}

struct Code {
    value: String,
    verifier: String,
}

struct Fixture {
    shard: Handle,
    origin: String,
    fetcher: Fetcher,
    fake: Arc<FakePds>,
    tasks: [JoinHandle<()>; 2],
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Fixture {
    async fn new() -> Self {
        let pds_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fake PDS can bind a loopback port");
        let tenor_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("tenor can bind a loopback port");
        let pds_origin = format!(
            "http://{}",
            pds_listener.local_addr().expect("bound PDS listener has an address")
        );
        let origin = format!(
            "http://{}",
            tenor_listener
                .local_addr()
                .expect("bound tenor listener has an address")
        );
        let fetcher = Fetcher::new(Policy::default().allow_private()).expect("explicit loopback policy builds");
        let shard = spawn(
            Shard::open(
                Arc::new(InMemory::new()),
                "oauth",
                Arc::new(MasterKey::from_bytes(&[83; 32])),
            )
            .await
            .expect("fresh in-memory OAuth shard opens"),
            Options {
                max_batch: 8,
                checkpoint_every: Duration::from_secs(3600),
            },
        );
        let world = shard
            .run(Initialize(origin.clone()))
            .await
            .expect("World initialization reaches the shard")
            .expect("fixture World initializes");
        let key = generate_key(KeyType::P256Private).expect("fixture P-256 key generation succeeds");
        let mut documents: BTreeMap<_, _> = [OWNER, WRITER, OUTSIDER]
            .into_iter()
            .map(|did| (did.into(), document(did, &pds_origin, &key)))
            .collect();
        documents.insert(WORLD.into(), world.document);
        let fake = Arc::new(FakePds {
            origin: pds_origin,
            fetcher: fetcher.clone(),
            documents,
            client_key: Arc::new(key),
            nonce: generate_state(),
            pars: Mutex::default(),
            codes: Mutex::default(),
            proofs: Mutex::default(),
            assertions: Mutex::default(),
            subject_override: Mutex::default(),
        });
        let pds_router = fake.router();
        let pds_task = tokio::spawn(async move {
            axum::serve(pds_listener, pds_router)
                .await
                .expect("fake PDS serves its bound listener");
        });
        let plc = format!("{}/plc", fake.origin);
        let resolver = Arc::new(DidResolver::new(fetcher.clone(), &plc));
        let hosting = Arc::new(
            Hosting::new(
                &origin.parse().expect("loopback tenor origin is an absolute URL"),
                vec![".motet.test".into()],
                plc,
                fetcher.clone(),
                Vec::new(),
            )
            .expect("fixture hosting configuration is valid"),
        );
        let audience = hosting.audience().to_owned();
        let router = api::router(
            Arc::new(api::State {
                shard: shard.clone(),
                records: pds::Pds::new(fetcher.clone(), Arc::<DidResolver>::clone(&resolver)),
                hosting,
            }),
            motet_xrpc::service_auth::authenticate(
                resolver,
                Arc::new(move |aud| api::accepts_audience(aud, &audience)),
            ),
        )
        .await
        .expect("real tenor router initializes OAuth and registers its methods");
        let tenor_task = tokio::spawn(async move {
            axum::serve(tenor_listener, router)
                .await
                .expect("tenor serves its bound listener");
        });
        Self {
            shard,
            origin,
            fetcher,
            fake,
            tasks: [pds_task, tenor_task],
        }
    }

    fn client(&self, confidential: bool) -> Client {
        let kind = if confidential { "confidential" } else { "public" };
        Client {
            id: format!("{}/client/{kind}.json", self.fake.origin),
            redirect: format!("{}/client/callback", self.fake.origin),
            signing: confidential.then(|| Arc::clone(&self.fake.client_key)),
            dpop: generate_key(KeyType::P256Private).expect("fixture P-256 key generation succeeds"),
        }
    }

    async fn get(&self, url: &str, cookie: Option<&str>) -> Reply {
        let mut request = self.fetcher.client().get(url);
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        Reply::send(request).await
    }

    async fn form(&self, path: &str, form: &Fields, proof: Option<&str>, cookie: Option<&str>) -> Reply {
        let mut request = self.fetcher.client().post(format!("{}{path}", self.origin)).form(form);
        if let Some(proof) = proof {
            request = request.header("dpop", proof);
        }
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        Reply::send(request).await
    }

    async fn oauth(&self, client: &Client, path: &str, mut form: Fields, key: &KeyData) -> Reply {
        client.authenticate(&mut form, &self.origin);
        let url = format!("{}{path}", self.origin);
        let (proof, _, _) = dpop::auth_dpop(key, "POST", &url).expect("fixture key signs an authorization proof");
        let challenged = self.form(path, &form, Some(&proof), None).await;
        let nonce = challenged.nonce(StatusCode::BAD_REQUEST);
        let (proof, _, _) = dpop::dpop_with_nonce(key, "POST", &url, None, nonce)
            .expect("fixture key signs the nonce-bound authorization proof");
        client.authenticate(&mut form, &self.origin);
        self.form(path, &form, Some(&proof), None).await
    }

    async fn begin(&self, client: &Client, scope: &str) -> Login {
        let (verifier, challenge) = pkce::generate();
        let state = generate_state();
        let response = self
            .oauth(
                client,
                "/oauth/par",
                client.par(scope, &challenge, &state),
                &client.dpop,
            )
            .await;
        response.expect(StatusCode::CREATED);
        self.authorize(client, &response.json(), verifier, state).await
    }

    async fn authorize(&self, client: &Client, par: &Value, verifier: String, state: String) -> Login {
        let mut authorize = Url::parse(&format!("{}/oauth/authorize", self.origin))
            .expect("tenor origin forms an authorization endpoint URL");
        authorize
            .query_pairs_mut()
            .append_pair("client_id", &client.id)
            .append_pair("request_uri", json_str(par, "request_uri"));
        let response = self.get(authorize.as_str(), None).await;
        response.expect(StatusCode::OK);
        Login {
            verifier,
            state,
            cookie: response.cookie(),
            form: fields([
                ("flow", &hidden(&response.body, "flow")),
                ("csrf", &hidden(&response.body, "csrf")),
            ]),
        }
    }

    async fn upstream(&self, mut login: Login, actor: &str) -> Callback {
        login.form.insert("identifier".into(), actor.into());
        let response = self.form("/oauth/login", &login.form, None, Some(&login.cookie)).await;
        let authorize = response.location();
        assert_eq!(
            authorize.origin(),
            Url::parse(&self.fake.origin)
                .expect("fake PDS origin is an absolute URL")
                .origin()
        );
        let response = self.get(authorize.as_str(), None).await;
        Callback {
            login,
            url: response.location(),
        }
    }

    async fn consent(&self, client: &Client, callback: Callback) -> Code {
        let response = self.get(callback.url.as_str(), Some(&callback.login.cookie)).await;
        response.expect(StatusCode::OK);
        let form = fields([
            ("flow", &hidden(&response.body, "flow")),
            ("csrf", &hidden(&response.body, "csrf")),
            ("decision", "approve"),
        ]);
        let approved = self
            .form("/oauth/consent", &form, None, Some(&callback.login.cookie))
            .await;
        let redirect = approved.location();
        let expected = Url::parse(&client.redirect).expect("registered client redirect is an absolute URL");
        assert_eq!(redirect.origin(), expected.origin());
        assert_eq!(redirect.path(), expected.path());
        let query: Fields = redirect.query_pairs().into_owned().collect();
        assert_eq!(query.get("state"), Some(&callback.login.state));
        assert_eq!(query.get("iss"), Some(&self.origin));
        Code {
            value: query
                .get("code")
                .expect("approved consent returns an authorization code")
                .clone(),
            verifier: callback.login.verifier,
        }
    }

    async fn code(&self, client: &Client, actor: &str, scope: &str) -> Code {
        let login = self.begin(client, scope).await;
        self.consent(client, self.upstream(login, actor).await).await
    }

    async fn token(&self, client: &Client, actor: &str, scope: &str) -> Value {
        let code = self.code(client, actor, scope).await;
        let response = self
            .oauth(client, "/oauth/token", client.exchange(&code), &client.dpop)
            .await;
        response.expect(StatusCode::OK);
        response.json()
    }

    async fn claims(&self, token: &Value) -> Claims {
        let access = json_str(token, "access_token");
        let header: Header = jwt_part(access, 0).expect("issued token has a JOSE header");
        let response = self
            .get(&format!("{}/.well-known/oauth-authorization-server", self.origin), None)
            .await;
        response.expect(StatusCode::OK);
        let metadata = response.json();
        let response = self.get(json_str(&metadata, "jwks_uri"), None).await;
        response.expect(StatusCode::OK);
        let keys: WrappedJsonWebKeySet = serde_json::from_value(response.json()).expect("AS publishes a valid JWKS");
        let public = keys
            .keys
            .iter()
            .find(|key| key.kid == header.key_id)
            .expect("AS publishes the access token signing key");
        jwt::verify(
            access,
            &jwk::to_key_data(public).expect("published JWK encodes an EC public key"),
        )
        .expect("session access JWT remains cryptographically valid")
    }

    async fn write(&self, method: &str, access: &str, proof: Option<&str>, body: &Value) -> Reply {
        let mut request = self
            .fetcher
            .client()
            .post(format!("{}/xrpc/{method}", self.origin))
            .header("authorization", format!("DPoP {access}"))
            .json(body);
        if let Some(proof) = proof {
            request = request.header("dpop", proof);
        }
        Reply::send(request).await
    }

    async fn resource_nonce(&self, client: &Client, method: &str, token: &Value, body: &Value) -> String {
        let access = json_str(token, "access_token");
        let url = format!("{}/xrpc/{method}", self.origin);
        let (proof, _, _) = dpop::request_dpop(&client.dpop, "POST", &url, access)
            .expect("fixture key signs a token-bound resource proof");
        let response = self.write(method, access, Some(&proof), body).await;
        response.nonce(StatusCode::UNAUTHORIZED).into()
    }

    async fn authorized_write(&self, client: &Client, method: &str, token: &Value, body: &Value) -> Reply {
        let nonce = self.resource_nonce(client, method, token, body).await;
        let access = json_str(token, "access_token");
        let (proof, _, _) = dpop::dpop_with_nonce(
            &client.dpop,
            "POST",
            &format!("{}/xrpc/{method}", self.origin),
            Some(access),
            &nonce,
        )
        .expect("fixture key signs a nonce-bound resource proof");
        self.write(method, access, Some(&proof), body).await
    }

    async fn audits(&self) -> Vec<Value> {
        let world = Did::new(WORLD);
        let space = spaces::world_space(&world, spaces::GMS).expect("fixture World has a GM Space URI");
        spaces::storage::ReadStorage::new(self.shard.reader(), &world)
            .list_records(&space, "at.motet.pilot.audit", None, 100, false)
            .await
            .expect("committed GM audit records are readable")
            .records
            .into_iter()
            .map(|row| atproto_dasl::from_slice_to_json(&row.value).expect("audit record contains valid DRISL"))
            .collect()
    }

    async fn record(&self, collection: &str, rkey: &str) -> Reply {
        let mut url = Url::parse(&format!("{}/xrpc/com.atproto.repo.getRecord", self.origin))
            .expect("tenor origin forms a getRecord endpoint URL");
        url.query_pairs_mut()
            .append_pair("repo", WORLD)
            .append_pair("collection", collection)
            .append_pair("rkey", rkey);
        self.get(url.as_str(), None).await
    }
}

fn character() -> Value {
    json!({"repo": WORLD, "collection": CHARACTER, "rkey": Tid::new().to_string(),
        "record": {"$type": CHARACTER, "name": "Aster", "createdAt": Datetime::now().into_string()}})
}

fn scope_set(token: &Value) -> BTreeSet<&str> {
    json_str(token, "scope").split_whitespace().collect()
}

/// A GM's own-PDS login grants a World write, never an unattributed owner write.
/// Catches confusing the upstream subject with the World, or committing without audit.
#[tokio::test]
async fn gm_login_consent_and_dpop_commit_a_world_record_with_actor_audit() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let token = fixture.token(&client, OWNER, "atproto repo:*").await;
    let claims = fixture.claims(&token).await;
    assert_eq!(claims.jose.subject.as_deref(), Some(WORLD));
    assert_eq!(claims.private["act"], OWNER);
    assert_eq!(
        claims.private["cnf"]["jkt"],
        jwk::thumbprint(&jwk::generate(&client.dpop).unwrap()).unwrap()
    );
    assert_eq!(token["token_type"], "DPoP");
    let body = character();
    let response = fixture.authorized_write(&client, CREATE, &token, &body).await;
    response.expect(StatusCode::OK);
    let created = response.json();
    let record = fixture.record(CHARACTER, body["rkey"].as_str().unwrap()).await;
    record.expect(StatusCode::OK);
    assert_eq!(record.json()["value"], body["record"]);
    assert_eq!(record.json()["cid"], created["cid"]);
    let audit = fixture.audits().await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["actor"], OWNER);
    assert_eq!(audit[0]["action"], "create");
    assert_eq!(audit[0]["subject"], created["uri"]);
}

/// Authority removal applies to the next write even before a signed JWT expires.
/// Catches checking membership only at consent, or treating a deleted role as live.
#[tokio::test]
async fn deleting_membership_or_role_refuses_the_next_still_valid_session_write() {
    for (collection, rkey) in [
        ("group.opensocial.membership", WRITER),
        ("group.opensocial.role", "writer"),
    ] {
        let fixture = Fixture::new().await;
        let client = fixture.client(false);
        let token = fixture.token(&client, WRITER, &format!("atproto {CREATE_SCOPE}")).await;
        fixture
            .authorized_write(&client, CREATE, &token, &character())
            .await
            .expect(StatusCode::OK);
        fixture
            .shard
            .run(RemoveAuthority { collection, rkey })
            .await
            .unwrap()
            .unwrap();
        let claims = fixture.claims(&token).await;
        assert_eq!(claims.private["act"], WRITER);
        let refused = character();
        fixture
            .authorized_write(&client, CREATE, &token, &refused)
            .await
            .expect(StatusCode::FORBIDDEN);
        assert!(
            !fixture
                .record(CHARACTER, refused["rkey"].as_str().unwrap())
                .await
                .status
                .is_success()
        );
        assert_eq!(
            fixture.audits().await.len(),
            1,
            "refusal must not add an audit or mutate the repo"
        );
    }
}

/// Possessing a token alone cannot write; each proof binds key, request and nonce once.
/// Catches ignored cnf/ath/htu/htm claims, unvalidated nonces, or replay bookkeeping gaps.
#[tokio::test]
async fn stolen_tokens_missing_stale_misbound_and_reused_proofs_cannot_write() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let token = fixture.token(&client, OWNER, "atproto repo:*").await;
    let access = token["access_token"].as_str().unwrap();
    let body = character();
    fixture
        .write(CREATE, access, None, &body)
        .await
        .expect(StatusCode::UNAUTHORIZED);
    let nonce = fixture.resource_nonce(&client, CREATE, &token, &body).await;
    let url = format!("{}/xrpc/{CREATE}", fixture.origin);
    let wrong_key = generate_key(KeyType::P256Private).expect("fixture P-256 key generation succeeds");
    for (key, method, target, bound_token, proof_nonce) in [
        (&wrong_key, "POST", url.as_str(), access, nonce.as_str()),
        (&client.dpop, "GET", url.as_str(), access, nonce.as_str()),
        (&client.dpop, "POST", fixture.origin.as_str(), access, nonce.as_str()),
        (
            &client.dpop,
            "POST",
            url.as_str(),
            "another-access-token",
            nonce.as_str(),
        ),
        (
            &client.dpop,
            "POST",
            url.as_str(),
            access,
            "nonce-no-longer-in-the-server-window",
        ),
    ] {
        let (proof, _, _) = dpop::dpop_with_nonce(key, method, target, Some(bound_token), proof_nonce).unwrap();
        fixture
            .write(CREATE, access, Some(&proof), &body)
            .await
            .expect(StatusCode::UNAUTHORIZED);
    }
    assert_eq!(fixture.audits().await, Vec::<Value>::new());
    assert!(
        !fixture
            .record(CHARACTER, body["rkey"].as_str().unwrap())
            .await
            .status
            .is_success()
    );
    let (proof, _, _) = dpop::dpop_with_nonce(&client.dpop, "POST", &url, Some(access), &nonce).unwrap();
    fixture
        .write(CREATE, access, Some(&proof), &body)
        .await
        .expect(StatusCode::OK);
    let replay_target = character();
    fixture
        .write(CREATE, access, Some(&proof), &replay_target)
        .await
        .expect(StatusCode::UNAUTHORIZED);
    assert!(
        !fixture
            .record(CHARACTER, replay_target["rkey"].as_str().unwrap())
            .await
            .status
            .is_success()
    );
    assert_eq!(fixture.audits().await.len(), 1);
}

/// An authenticated outsider cannot reach World consent or obtain an authorization code.
/// Catches trusting a successful PDS login without consulting the World's membership.
#[tokio::test]
async fn nonmembers_cannot_obtain_consent_or_a_code() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let callback = fixture
        .upstream(fixture.begin(&client, "atproto repo:*").await, OUTSIDER)
        .await;
    let response = fixture.get(callback.url.as_str(), Some(&callback.login.cookie)).await;
    response.expect(StatusCode::FORBIDDEN);
    assert_eq!(response.json()["error"], "access_denied");
    assert!(!response.headers.contains_key("location"));
    assert_eq!(fixture.audits().await, Vec::<Value>::new());
}

/// Consent and writes intersect the actor's grants with the session's collection/action scopes.
/// Catches widening grants at consent or letting an owner bypass a narrower session.
#[tokio::test]
async fn excessive_scopes_are_omitted_and_collection_and_action_limits_survive_issuance() {
    let fixture = Fixture::new().await;
    let excessive = format!(
        "atproto {CREATE_SCOPE} repo:* repo:at.motet.pilot.profile?action=create repo:at.motet.pilot.character?action=delete"
    );
    let narrow = format!("atproto {CREATE_SCOPE}");
    for (actor, requested) in [(WRITER, excessive.as_str()), (OWNER, narrow.as_str())] {
        let client = fixture.client(false);
        let token = fixture.token(&client, actor, requested).await;
        assert_eq!(scope_set(&token), BTreeSet::from(["atproto", CREATE_SCOPE]));
        let claims = fixture.claims(&token).await;
        assert_eq!(claims.private["scope"], token["scope"]);
        let body = character();
        fixture
            .authorized_write(&client, CREATE, &token, &body)
            .await
            .expect(StatusCode::OK);
        let profile = json!({"repo": WORLD, "collection": PROFILE, "rkey": "self", "record": {
            "$type": PROFILE, "createdAt": Datetime::now().into_string(), "displayName": "Not permitted",
        }});
        fixture
            .authorized_write(&client, CREATE, &token, &profile)
            .await
            .expect(StatusCode::FORBIDDEN);
        let delete = json!({"repo": WORLD, "collection": CHARACTER, "rkey": body["rkey"]});
        fixture
            .authorized_write(&client, DELETE, &token, &delete)
            .await
            .expect(StatusCode::FORBIDDEN);
        fixture
            .record(CHARACTER, body["rkey"].as_str().unwrap())
            .await
            .expect(StatusCode::OK);
        assert!(!fixture.record(PROFILE, "self").await.status.is_success());
    }
    assert_eq!(fixture.audits().await.len(), 2);
}

/// Rotation preserves session bindings; authenticated reuse revokes every token in its family.
/// Catches discarded refresh tombstones, rolled-back revocation, or wrong-key/client denial of service.
#[tokio::test]
async fn refresh_rotation_preserves_bindings_and_authenticated_reuse_revokes_the_family() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let token = fixture.token(&client, WRITER, &format!("atproto {CREATE_SCOPE}")).await;
    let wrong_key = generate_key(KeyType::P256Private).expect("fixture P-256 key generation succeeds");
    let refused = fixture
        .oauth(&client, "/oauth/token", client.refresh(&token), &wrong_key)
        .await;
    refused.expect(StatusCode::BAD_REQUEST);
    let mut refresh = client.refresh(&token);
    refresh.insert("scope".into(), "atproto repo:*".into());
    let response = fixture.oauth(&client, "/oauth/token", refresh, &client.dpop).await;
    response.expect(StatusCode::OK);
    let rotated = response.json();
    assert_ne!(rotated["refresh_token"], token["refresh_token"]);
    assert_eq!(scope_set(&rotated), scope_set(&token));
    let before = fixture.claims(&token).await;
    let after = fixture.claims(&rotated).await;
    assert_eq!(after.jose.subject, before.jose.subject);
    assert_eq!(after.private["act"], WRITER);
    assert_eq!(after.private["cnf"], before.private["cnf"]);
    assert_eq!(after.private["scope"], before.private["scope"]);
    let other_client = fixture.client(true);
    for (request_client, key) in [(&client, &wrong_key), (&other_client, &client.dpop)] {
        let rejected = fixture
            .oauth(request_client, "/oauth/token", request_client.refresh(&token), key)
            .await;
        rejected.expect(StatusCode::BAD_REQUEST);
        assert_eq!(rejected.json()["error"], "invalid_grant");
    }
    fixture
        .authorized_write(&client, CREATE, &rotated, &character())
        .await
        .expect(StatusCode::OK);
    let replay = fixture
        .oauth(&client, "/oauth/token", client.refresh(&token), &client.dpop)
        .await;
    replay.expect(StatusCode::BAD_REQUEST);
    assert_eq!(replay.json()["error"], "invalid_grant");
    let successor = fixture
        .oauth(&client, "/oauth/token", client.refresh(&rotated), &client.dpop)
        .await;
    successor.expect(StatusCode::BAD_REQUEST);
    assert_eq!(successor.json()["error"], "invalid_grant");
    for issued in [&token, &rotated] {
        fixture.claims(issued).await;
        let refused = character();
        fixture
            .authorized_write(&client, CREATE, issued, &refused)
            .await
            .expect(StatusCode::UNAUTHORIZED);
        assert!(
            !fixture
                .record(CHARACTER, json_str(&refused, "rkey"))
                .await
                .status
                .is_success()
        );
    }
    let audit = fixture.audits().await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["actor"], WRITER);
}

/// Confidential metadata cannot downgrade into public authentication at PAR or token exchange.
/// Catches missing-key fallback, accepting an unrelated signature, or ignoring assertion audience/replay.
#[tokio::test]
async fn public_and_confidential_client_authentication_fail_closed() {
    let fixture = Fixture::new().await;
    let client = fixture.client(true);
    let (verifier, challenge) = pkce::generate();
    let state = generate_state();
    let form = client.par("atproto repo:*", &challenge, &state);
    // PAR permits no proof; authentication must still be enforced before creating a request URI.
    let missing = fixture.form("/oauth/par", &form, None, None).await;
    missing.expect(StatusCode::UNAUTHORIZED);
    assert_eq!(missing.json()["error"], "invalid_client");
    let wrong_key = generate_key(KeyType::P256Private).expect("fixture P-256 key generation succeeds");
    for (key, audience) in [
        (&wrong_key, fixture.origin.as_str()),
        (
            client.signing.as_deref().unwrap(),
            "https://another-authorization-server.example",
        ),
    ] {
        let mut invalid = form.clone();
        invalid.insert("client_assertion_type".into(), ASSERTION_TYPE.into());
        invalid.insert("client_assertion".into(), client_assertion(key, &client.id, audience));
        fixture
            .form("/oauth/par", &invalid, None, None)
            .await
            .expect(StatusCode::UNAUTHORIZED);
    }
    let mut authenticated = form;
    client.authenticate(&mut authenticated, &fixture.origin);
    fixture
        .form("/oauth/par", &authenticated, None, None)
        .await
        .expect(StatusCode::CREATED);
    fixture
        .form("/oauth/par", &authenticated, None, None)
        .await
        .expect(StatusCode::UNAUTHORIZED);
    let public = fixture.client(false);
    let mut confused = public.par("atproto repo:*", &challenge, &state);
    confused.insert("client_assertion_type".into(), ASSERTION_TYPE.into());
    confused.insert(
        "client_assertion".into(),
        client_assertion(&wrong_key, &public.id, &fixture.origin),
    );
    fixture
        .form("/oauth/par", &confused, None, None)
        .await
        .expect(StatusCode::UNAUTHORIZED);
    let mut unsupported = public.par("atproto repo:*", &challenge, &state);
    unsupported.insert(
        "client_id".into(),
        format!("{}/client/secret.json", fixture.fake.origin),
    );
    fixture
        .form("/oauth/par", &unsupported, None, None)
        .await
        .expect(StatusCode::BAD_REQUEST);
    let code = fixture.code(&client, OWNER, "atproto repo:*").await;
    let exchange = client.exchange(&code);
    fixture
        .form("/oauth/token", &exchange, None, None)
        .await
        .expect(StatusCode::BAD_REQUEST);
    let wrong_binding = fixture
        .oauth(&client, "/oauth/token", client.exchange(&code), &wrong_key)
        .await;
    wrong_binding.expect(StatusCode::BAD_REQUEST);
    let url = format!("{}/oauth/token", fixture.origin);
    let (proof, _, _) = dpop::auth_dpop(&client.dpop, "POST", &url).unwrap();
    let challenge = fixture.form("/oauth/token", &exchange, Some(&proof), None).await;
    let nonce = challenge.nonce(StatusCode::BAD_REQUEST);
    let (proof, _, _) = dpop::dpop_with_nonce(&client.dpop, "POST", &url, None, nonce).unwrap();
    fixture
        .form("/oauth/token", &exchange, Some(&proof), None)
        .await
        .expect(StatusCode::UNAUTHORIZED);
    let mut wrong_pkce = client.exchange(&code);
    wrong_pkce.insert("code_verifier".into(), verifier);
    fixture
        .oauth(&client, "/oauth/token", wrong_pkce, &client.dpop)
        .await
        .expect(StatusCode::BAD_REQUEST);
    let response = fixture.oauth(&client, "/oauth/token", exchange, &client.dpop).await;
    response.expect(StatusCode::OK);
    fixture
        .authorized_write(&client, CREATE, &response.json(), &character())
        .await
        .expect(StatusCode::OK);
}

/// The GM is the account authenticated by the originally discovered server, not callback input.
/// Catches skipped RFC 9207 issuer checks or accepting a substituted token endpoint subject.
#[tokio::test]
async fn upstream_issuer_and_subject_substitution_never_reach_world_consent() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let mut callback = fixture
        .upstream(fixture.begin(&client, "atproto repo:*").await, OWNER)
        .await;
    let mut query: Fields = callback.url.query_pairs().into_owned().collect();
    query.insert("iss".into(), "https://wrong-issuer.example".into());
    callback
        .url
        .set_query(Some(&serde_urlencoded::to_string(&query).unwrap()));
    let response = fixture.get(callback.url.as_str(), Some(&callback.login.cookie)).await;
    assert!(response.status.is_client_error(), "{}", response.body);
    assert!(!response.headers.contains_key("location"));
    *fixture
        .fake
        .subject_override
        .lock()
        .expect("fixture mutex is not poisoned") = Some(OWNER);
    let callback = fixture
        .upstream(fixture.begin(&client, &format!("atproto {CREATE_SCOPE}")).await, WRITER)
        .await;
    let response = fixture.get(callback.url.as_str(), Some(&callback.login.cookie)).await;
    assert!(response.status.is_client_error(), "{}", response.body);
    assert!(!response.headers.contains_key("location"));
    assert_eq!(fixture.audits().await, Vec::<Value>::new());
}

/// Fractional `NumericDates` remain usable through every `DPoP` boundary and remain single-use.
/// Catches a second integer-only claims decoder rejecting an already verified proof.
#[tokio::test]
async fn fractional_dpop_dates_complete_the_flow_without_weakening_replay_protection() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let (verifier, challenge) = pkce::generate();
    let state = generate_state();
    let form = client.par("atproto repo:*", &challenge, &state);
    let par_url = format!("{}/oauth/par", fixture.origin);
    let (proof, _, _) = dpop::auth_dpop(&client.dpop, "POST", &par_url).unwrap();
    let challenged = fixture
        .form("/oauth/par", &form, Some(&fractional_proof(&client.dpop, &proof)), None)
        .await;
    let nonce = challenged.nonce(StatusCode::BAD_REQUEST);
    let (proof, _, _) = dpop::dpop_with_nonce(&client.dpop, "POST", &par_url, None, nonce).unwrap();
    let proof = fractional_proof(&client.dpop, &proof);
    let response = fixture.form("/oauth/par", &form, Some(&proof), None).await;
    response.expect(StatusCode::CREATED);
    let replay = fixture.form("/oauth/par", &form, Some(&proof), None).await;
    replay.expect(StatusCode::BAD_REQUEST);
    assert_eq!(replay.json()["error"], "invalid_dpop_proof");
    let login = fixture.authorize(&client, &response.json(), verifier, state).await;
    let callback = fixture.upstream(login, OWNER).await;
    let code = fixture.consent(&client, callback).await;
    let exchange = client.exchange(&code);
    let token_url = format!("{}/oauth/token", fixture.origin);
    let (proof, _, _) = dpop::auth_dpop(&client.dpop, "POST", &token_url).unwrap();
    let challenged = fixture
        .form(
            "/oauth/token",
            &exchange,
            Some(&fractional_proof(&client.dpop, &proof)),
            None,
        )
        .await;
    let nonce = challenged.nonce(StatusCode::BAD_REQUEST);
    let (proof, _, _) = dpop::dpop_with_nonce(&client.dpop, "POST", &token_url, None, nonce).unwrap();
    let response = fixture
        .form(
            "/oauth/token",
            &exchange,
            Some(&fractional_proof(&client.dpop, &proof)),
            None,
        )
        .await;
    response.expect(StatusCode::OK);
    let token = response.json();
    let access = token["access_token"].as_str().unwrap();
    let body = character();
    let resource_url = format!("{}/xrpc/{CREATE}", fixture.origin);
    let (proof, _, _) = dpop::request_dpop(&client.dpop, "POST", &resource_url, access).unwrap();
    let challenged = fixture
        .write(CREATE, access, Some(&fractional_proof(&client.dpop, &proof)), &body)
        .await;
    let nonce = challenged.nonce(StatusCode::UNAUTHORIZED);
    let (proof, _, _) = dpop::dpop_with_nonce(&client.dpop, "POST", &resource_url, Some(access), nonce).unwrap();
    let proof = fractional_proof(&client.dpop, &proof);
    fixture
        .write(CREATE, access, Some(&proof), &body)
        .await
        .expect(StatusCode::OK);
    let replay_target = character();
    fixture
        .write(CREATE, access, Some(&proof), &replay_target)
        .await
        .expect(StatusCode::UNAUTHORIZED);
    fixture
        .record(CHARACTER, body["rkey"].as_str().unwrap())
        .await
        .expect(StatusCode::OK);
    assert!(
        !fixture
            .record(CHARACTER, replay_target["rkey"].as_str().unwrap())
            .await
            .status
            .is_success()
    );
    let audit = fixture.audits().await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["actor"], OWNER);
}

/// A browser client can discover the AS and read nonce challenges without sending cookies.
/// Catches missing OPTIONS handling or hiding `DPoP-Nonce`/error bodies behind CORS.
#[tokio::test]
async fn public_oauth_cors_allows_discovery_preflight_and_nonce_challenge_recovery() {
    let fixture = Fixture::new().await;
    let origin = "https://independent-world-editor.example";
    for path in [
        "/.well-known/oauth-authorization-server",
        "/.well-known/oauth-protected-resource",
        "/oauth/client-metadata.json",
        "/oauth/jwks.json",
    ] {
        let response = Reply::send(
            fixture
                .fetcher
                .client()
                .get(format!("{}{path}", fixture.origin))
                .header("origin", origin),
        )
        .await;
        response.expect(StatusCode::OK);
        response.public_cors(origin);
        assert!(response.json().is_object());
    }
    for path in ["/oauth/par", "/oauth/token"] {
        let response = Reply::send(
            fixture
                .fetcher
                .client()
                .request(reqwest::Method::OPTIONS, format!("{}{path}", fixture.origin))
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "content-type,dpop"),
        )
        .await;
        assert!(response.status.is_success(), "{}", response.body);
        response.public_cors(origin);
        assert!(header_allows(&response.headers, "access-control-allow-methods", "POST"));
        for header in ["content-type", "dpop"] {
            assert!(header_allows(&response.headers, "access-control-allow-headers", header));
        }
    }
    let client = fixture.client(false);
    let code = fixture.code(&client, OWNER, "atproto repo:*").await;
    let (_, challenge) = pkce::generate();
    for (path, form, status) in [
        (
            "/oauth/par",
            client.par("atproto repo:*", &challenge, &generate_state()),
            StatusCode::CREATED,
        ),
        ("/oauth/token", client.exchange(&code), StatusCode::OK),
    ] {
        let url = format!("{}{path}", fixture.origin);
        let (proof, _, _) = dpop::auth_dpop(&client.dpop, "POST", &url).unwrap();
        let response = Reply::send(
            fixture
                .fetcher
                .client()
                .post(&url)
                .header("origin", origin)
                .header("dpop", proof)
                .form(&form),
        )
        .await;
        response.public_cors(origin);
        assert!(header_allows(
            &response.headers,
            "access-control-expose-headers",
            "dpop-nonce"
        ));
        let nonce = response.nonce(StatusCode::BAD_REQUEST);
        let (proof, _, _) = dpop::dpop_with_nonce(&client.dpop, "POST", &url, None, nonce).unwrap();
        let recovered = Reply::send(
            fixture
                .fetcher
                .client()
                .post(&url)
                .header("origin", origin)
                .header("dpop", proof)
                .form(&form),
        )
        .await;
        recovered.expect(status);
        recovered.public_cors(origin);
    }
}

/// Script-free login and consent stay private to the browser's binding cookie.
/// Catches applying public OAuth CORS to routes that carry browser-authorized state.
#[tokio::test]
async fn cookie_bound_login_and_consent_do_not_become_cross_origin_credential_apis() {
    let fixture = Fixture::new().await;
    let client = fixture.client(false);
    let origin = "https://unrelated-page.example";
    for path in ["/oauth/login", "/oauth/consent"] {
        let response = Reply::send(
            fixture
                .fetcher
                .client()
                .request(reqwest::Method::OPTIONS, format!("{}{path}", fixture.origin))
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "content-type"),
        )
        .await;
        response.private_cors();
    }
    let mut login = fixture.begin(&client, "atproto repo:*").await;
    login.form.insert("identifier".into(), OWNER.into());
    let response = Reply::send(
        fixture
            .fetcher
            .client()
            .post(format!("{}/oauth/login", fixture.origin))
            .header("origin", origin)
            .header("cookie", &login.cookie)
            .form(&login.form),
    )
    .await;
    response.private_cors();
    let authorize = response.location();
    let response = fixture.get(authorize.as_str(), None).await;
    let callback = response.location();
    let response = fixture.get(callback.as_str(), Some(&login.cookie)).await;
    response.expect(StatusCode::OK);
    let form = fields([
        ("flow", &hidden(&response.body, "flow")),
        ("csrf", &hidden(&response.body, "csrf")),
        ("decision", "approve"),
    ]);
    let response = Reply::send(
        fixture
            .fetcher
            .client()
            .post(format!("{}/oauth/consent", fixture.origin))
            .header("origin", origin)
            .header("cookie", &login.cookie)
            .form(&form),
    )
    .await;
    response.private_cors();
    assert!(response.location().query_pairs().any(|(name, _)| name == "code"));
}

/// Standards-valid client assertions authorize the same client without weakening audience or replay checks.
/// Catches integer-only `NumericDate` or string-only audience decoding after signature verification.
#[tokio::test]
async fn fractional_client_assertions_with_audience_arrays_keep_client_binding() {
    let fixture = Fixture::new().await;
    let client = fixture.client(true);
    let signing = client.signing.as_deref().unwrap();
    let (verifier, challenge) = pkce::generate();
    let state = generate_state();
    let mut form = client.par("atproto repo:*", &challenge, &state);
    form.insert("client_assertion_type".into(), ASSERTION_TYPE.into());
    form.insert(
        "client_assertion".into(),
        fractional_assertion(signing, &client.id, &["https://unrelated-issuer.example"]),
    );
    let wrong_audience = fixture.form("/oauth/par", &form, None, None).await;
    wrong_audience.expect(StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_audience.json()["error"], "invalid_client");
    form.insert(
        "client_assertion".into(),
        fractional_assertion(signing, &client.id, &[fixture.origin.as_str()]),
    );
    let accepted = fixture.form("/oauth/par", &form, None, None).await;
    accepted.expect(StatusCode::CREATED);
    let replay = fixture.form("/oauth/par", &form, None, None).await;
    replay.expect(StatusCode::UNAUTHORIZED);
    assert_eq!(replay.json()["error"], "invalid_client");
    let login = fixture.authorize(&client, &accepted.json(), verifier, state).await;
    let callback = fixture.upstream(login, OWNER).await;
    let code = fixture.consent(&client, callback).await;
    let token = fixture
        .oauth(&client, "/oauth/token", client.exchange(&code), &client.dpop)
        .await;
    token.expect(StatusCode::OK);
    fixture
        .authorized_write(&client, CREATE, &token.json(), &character())
        .await
        .expect(StatusCode::OK);
    let audit = fixture.audits().await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["actor"], OWNER);
}
