//! Public profile sync crosses the real tenor's credential, signature and MAC boundary.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use atproto_dasl::{BlockStorage as _, Cid};
use atproto_identity::key::to_public;
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_record::datetime::Datetime;
use atproto_space::space_repo::OpAction;
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use motet_appview::archive::Archive;
use motet_appview::event::{Action, Event, RecordEvent};
use motet_appview::indexer::Indexer;
use motet_appview::{MIGRATOR, projection, public_space};
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite, http};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_xrpc::server::Service;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Mutex;

const WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:plc:oooooooooooooooooooooooo";
const PROFILE: &str = "group.opensocial.profile";
const DECLARATION: &str = "group.opensocial.declaration";

struct Initialize {
    endpoint: String,
    public: bool,
}

impl Op for Initialize {
    type Output = Document;
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Document, SpaceError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("fresh fixture World");
        let signing = txn.generate_key(&world, Purpose::Signing).expect("World signing key");
        let kem = txn.generate_key(&world, Purpose::Kem).expect("World KEM key");
        let signing = to_public(&signing).expect("public signing key").to_string();
        let kem = to_public(&kem).expect("public KEM key").to_string();
        let document: Document = serde_json::from_value(json!({
            "id": WORLD,
            "verificationMethod": [
                {"id": format!("{WORLD}#atproto"), "type": "Multikey", "controller": WORLD,
                 "publicKeyMultibase": signing.strip_prefix("did:key:").expect("multibase signing key")},
                {"id": format!("{WORLD}#atproto_kem"), "type": "Multikey", "controller": WORLD,
                 "publicKeyMultibase": kem.strip_prefix("did:key:").expect("multibase KEM key")}
            ],
            "service": [{"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": self.endpoint}]
        }))?;
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: "world.test".into(),
            epoch: 1,
            status: Status::Active,
            created: Datetime::now().into_string(),
            document: document.clone(),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        motet_tenor::repo::genesis(txn, &world).await?;
        spaces::bootstrap(
            txn,
            &world,
            &Did::new(OWNER),
            ProfileInput {
                display_name: Some(
                    if self.public {
                        "First public name"
                    } else {
                        "Never public secret"
                    }
                    .into(),
                ),
                public: self.public,
                ..ProfileInput::default()
            },
        )
        .await?;
        Ok(document)
    }
}

struct Write(Vec<SpaceWrite>);

impl Op for Write {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let meta = spaces::world_space(&Did::new(WORLD), spaces::META)?;
        spaces::write_as_world(txn, &meta, self.0).await?;
        Ok(())
    }
}

struct Directory(Document);

#[async_trait]
impl IdentityResolver for Directory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        anyhow::ensure!(subject == self.0.id, "unexpected DID");
        Ok(self.0.clone())
    }
}

struct Fixture {
    handle: Handle,
    client: public_space::Client,
    corruption: Arc<AtomicU8>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new(public: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind tenor HTTP");
        let endpoint = format!("http://{}", listener.local_addr().expect("local address"));
        let shard = Shard::open(
            Arc::new(InMemory::new()),
            "space-fixture",
            Arc::new(MasterKey::from_bytes(&[47; 32])),
        )
        .await
        .expect("open tenor shard");
        let handle = spawn(
            shard,
            Options {
                max_batch: 8,
                checkpoint_every: Duration::from_secs(3600),
            },
        );
        let document = handle
            .run(Initialize { endpoint, public })
            .await
            .expect("submit World bootstrap")
            .expect("bootstrap World");
        let resolver: Arc<dyn IdentityResolver> = Arc::new(Directory(document));
        let fetcher = Fetcher::new(Policy::default().allow_private()).expect("fixture network policy");
        let state = Arc::new(http::State {
            shard: handle.clone(),
            resolver: Arc::clone(&resolver),
            fetcher: fetcher.clone(),
        });
        let auth = motet_xrpc::service_auth::authenticate(Arc::clone(&resolver), Arc::new(|aud| aud == WORLD));
        let corruption = Arc::new(AtomicU8::new(0));
        let middleware_corruption = Arc::clone(&corruption);
        let router = http::register(Service::new(http::authenticate(auth, Arc::clone(&state))), &state)
            .into_router(())
            .expect("Space methods registered")
            .layer(axum::middleware::from_fn(http::vary_mac))
            .layer(axum::middleware::from_fn(move |request: Request, next: Next| {
                let corruption = Arc::clone(&middleware_corruption);
                async move {
                    let is_record = request.uri().path() == "/xrpc/com.atproto.space.getRecord";
                    let mut response = next.run(request).await;
                    if is_record && response.status().is_success() {
                        match corruption.load(Ordering::SeqCst) {
                            1 => {
                                response.headers_mut().remove(atproto_space::mac::MAC_HEADER);
                            }
                            2 => {
                                let (mut parts, body) = response.into_parts();
                                let bytes = to_bytes(body, 4 * 1024 * 1024)
                                    .await
                                    .expect("record response fits fixture limit");
                                let mut record: Value =
                                    serde_json::from_slice(&bytes).expect("record response is JSON");
                                *record
                                    .get_mut("value")
                                    .and_then(|value| value.get_mut("displayName"))
                                    .expect("profile has a display name") = "Forged name".into();
                                parts.headers.remove("content-length");
                                response = Response::from_parts(
                                    parts,
                                    Body::from(serde_json::to_vec(&record).expect("forged record serializes")),
                                );
                            }
                            _ => {}
                        }
                    }
                    response
                }
            }));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.expect("tenor HTTP server") });
        let client = public_space::Client::new(fetcher, resolver);
        Self {
            handle,
            client,
            corruption,
            server,
        }
    }

    async fn write(&self, mut write: SpaceWrite, action: OpAction) {
        write.action = action;
        self.handle
            .run(Write(vec![write]))
            .await
            .expect("submit Space write")
            .expect("write Space record");
    }

    async fn rename(&self, name: &str) {
        let write = SpaceWrite::create(PROFILE, "self", &json!({"displayName": name, "joinPolicy": "invite"}))
            .expect("profile encodes");
        self.write(write, OpAction::Update).await;
    }

    async fn set_public(&self, public: bool) {
        let write = SpaceWrite::create(
            "group.opensocial.access",
            "self",
            &json!({"public": public, "grants": [{"role": "owner", "scopes": ["space:*?collection=*"]}]}),
        )
        .expect("access record encodes");
        self.write(write, OpAction::Update).await;
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn name(pool: &PgPool) -> Option<String> {
    sqlx::query_scalar("SELECT display_name FROM projection.worlds WHERE did = $1")
        .bind(WORLD)
        .fetch_optional(pool)
        .await
        .expect("query projected World name")
        .flatten()
}

async fn archived(archive: &Archive) -> Vec<Event> {
    archive
        .read_from(0)
        .await
        .expect("read archived events")
        .into_iter()
        .flat_map(|(_, events)| events)
        .collect()
}

// Tap supplies these facts from the public repo. Read the declaration and its
// CID from the real bootstrap rather than constructing a parallel fixture.
async fn deliver_declaration(indexer: &Arc<Mutex<Indexer>>, fixture: &Fixture) {
    let world = Did::new(WORLD);
    let reader = fixture.handle.reader();
    let rev = motet_tenor::repo::head(reader, &world)
        .await
        .expect("read public repo head")
        .expect("World has a repo head")
        .rev
        .to_string();
    let index = reader
        .get(&keys::repo_record(&world, DECLARATION, "self"))
        .await
        .expect("read declaration index");
    let (action, cid, record) = if let Some(index) = index {
        let cid = Cid::from_bytes(&index).expect("declaration index has a CID");
        let bytes = reader
            .blocks(&world)
            .get(&cid)
            .await
            .expect("read declaration block")
            .expect("declaration block exists");
        (
            Action::Create,
            Some(cid.to_string()),
            Some(atproto_dasl::from_slice_to_json(&bytes).expect("declaration is DRISL")),
        )
    } else {
        (Action::Delete, None, None)
    };
    indexer
        .lock()
        .await
        .ingest(&[Event::Record(RecordEvent {
            did: WORLD.into(),
            rev,
            collection: DECLARATION.into(),
            rkey: "self".into(),
            action,
            cid,
            record,
        })])
        .await
        .expect("archive declaration");
}

/// Reopening a formerly private Space must not archive its old private names;
/// a stale declaration or a skipped signature/MAC would leak or lose metadata.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn only_current_public_profiles_are_archived_and_rebuild_to_the_same_name() {
    let fixture = Fixture::new(true).await;
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.unwrap();
    let archive = Archive::new(Arc::new(InMemory::new()), "appview-archive");
    let indexer = Arc::new(Mutex::new(Indexer::start(pool.clone(), &archive).await.unwrap()));
    deliver_declaration(&indexer, &fixture).await;

    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(name(&pool).await.as_deref(), Some("First public name"));
    let before = archived(&archive).await.len();
    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(
        archived(&archive).await.len(),
        before,
        "unchanged polls do not grow the archive"
    );

    fixture.rename("Renamed public World").await;
    // A network intermediary that drops the host MAC or tampers with the
    // value must not change the name or append an unverified archive record.
    for corruption in [1, 2] {
        fixture.corruption.store(corruption, Ordering::SeqCst);
        public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
        assert_eq!(name(&pool).await.as_deref(), Some("First public name"));
        assert_eq!(archived(&archive).await.len(), before);
    }
    fixture.corruption.store(0, Ordering::SeqCst);
    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(name(&pool).await.as_deref(), Some("Renamed public World"));

    fixture.set_public(false).await;
    fixture.rename("Private draft one").await;
    fixture.rename("Private draft two").await;
    let before = archived(&archive).await.len();
    // Tap's public declaration deletion has not arrived yet. The tenor must
    // refuse the anonymous mint rather than disclose the private profile.
    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(archived(&archive).await.len(), before);
    assert_eq!(name(&pool).await.as_deref(), Some("Renamed public World"));
    deliver_declaration(&indexer, &fixture).await;
    assert_eq!(
        name(&pool).await,
        None,
        "the declaration deletion immediately hides the public name"
    );
    projection::rebuild(&pool, &archive).await.unwrap();
    assert_eq!(
        name(&pool).await,
        None,
        "replay also hides the private World's former name"
    );

    fixture.rename("New public name").await;
    fixture.set_public(true).await;
    deliver_declaration(&indexer, &fixture).await;
    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(name(&pool).await.as_deref(), Some("New public name"));
    let events = archived(&archive).await;
    let text = serde_json::to_string(&events).unwrap();
    assert!(!text.contains("Private draft one") && !text.contains("Private draft two"));
    assert!(text.contains("New public name"));
    let live: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(r) FROM projection.space_records r ORDER BY space, did")
        .fetch_all(&pool)
        .await
        .unwrap();
    projection::rebuild(&pool, &archive).await.unwrap();
    let rebuilt: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM projection.space_records r ORDER BY space, did")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rebuilt, live);
    assert_eq!(name(&pool).await.as_deref(), Some("New public name"));

    let delete = SpaceWrite {
        action: OpAction::Delete,
        collection: PROFILE.into(),
        rkey: "self".into(),
        value: None,
        swap: None,
    };
    fixture.write(delete, OpAction::Delete).await;
    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(name(&pool).await, None, "a public profile deletion removes its name");
    projection::rebuild(&pool, &archive).await.unwrap();
    assert_eq!(name(&pool).await, None, "replay retains the deletion");
}

/// A public scene does not authorize a private World's meta Space. Catches
/// treating scene discovery as permission to fetch or archive its profile.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn a_public_scene_never_exposes_a_private_only_world_profile() {
    let fixture = Fixture::new(false).await;
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.unwrap();
    let archive = Archive::new(Arc::new(InMemory::new()), "private-world-archive");
    let indexer = Arc::new(Mutex::new(Indexer::start(pool.clone(), &archive).await.unwrap()));
    let world = Did::new(WORLD);
    let reader = fixture.handle.reader();
    assert!(
        reader
            .get(&keys::repo_record(&world, DECLARATION, "self"))
            .await
            .unwrap()
            .is_none()
    );
    let rev = motet_tenor::repo::head(reader, &world)
        .await
        .unwrap()
        .unwrap()
        .rev
        .to_string();
    // A public scene makes the World discoverable, but does not publish meta.
    indexer
        .lock()
        .await
        .ingest(&[Event::Record(RecordEvent {
            did: WORLD.into(),
            rev,
            collection: "at.motet.pilot.scene".into(),
            rkey: "room".into(),
            action: Action::Create,
            cid: Some("scene".into()),
            record: Some(json!({"$type": "at.motet.pilot.scene", "title": "Public scene"})),
        })])
        .await
        .unwrap();
    public_space::sync(&indexer, &pool, &fixture.client).await.unwrap();
    assert_eq!(name(&pool).await, None);
    let events = archived(&archive).await;
    assert!(events.iter().all(|event| !matches!(event, Event::SpaceRecord(_))));
    assert!(!serde_json::to_string(&events).unwrap().contains("Never public secret"));
}
