//! XRPC serving for the AppView and the tenor.
//!
//! Lexicons don't carry authorization, and NodeBB's worst bugs were an RPC
//! layer where every method was remotely callable and each handler had to
//! remember its own check. Here every method is declared once, in a
//! [`Registry`], with the [`Access`] it requires; a method that isn't
//! declared doesn't exist, and a declared one can't be reached without its
//! check passing. Handlers receive an already-authorized [`Caller`].
//!
//! Independent clients reach these methods the standard way: through their
//! PDS with the `atproto-proxy` header, which mints a service-auth token
//! naming the caller, this service and the method (`lxm`); [`service_auth`]
//! verifies it. A DID inside a request body is never the caller.

use std::collections::BTreeMap;

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

pub mod server;
pub mod service_auth;

pub use atproto_identity::traits::IdentityResolver;

/// Query (GET) or procedure (POST), as the lexicon declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Query,
    Procedure,
}

/// What a caller must present to reach a method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Access {
    /// Anyone, signed in or not. Results are still filtered per reader.
    Public,
    /// A signed-in Motet session (the web app's BFF cookie), or service auth
    /// from the account's own PDS.
    Account,
    /// A verified OAuth World session only. An account or service token is not a session.
    Session,
    /// Service auth only: another service acting for an account, with `lxm`
    /// bound to this method.
    Service,
    /// Either service auth bound to this method or a verified World session.
    ServiceOrSession,
    /// An authority-signed Space credential and its bound HTTP Message
    /// Signature. Handlers also check its exact Space and audience.
    SpaceCredential,
}

/// One XRPC method.
#[derive(Clone, Debug)]
pub struct Method {
    pub nsid: &'static str,
    pub kind: Kind,
    pub access: Access,
    /// Largest accepted request body.
    pub max_body: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("method {0} is declared twice")]
    Duplicate(&'static str),
}

/// Every method a service serves, and nothing else.
#[derive(Debug, Default)]
pub struct Registry {
    methods: BTreeMap<&'static str, Method>,
}

impl Registry {
    pub fn new(methods: impl IntoIterator<Item = Method>) -> Result<Self, RegistryError> {
        let mut out = BTreeMap::new();
        for m in methods {
            if out.insert(m.nsid, m.clone()).is_some() {
                return Err(RegistryError::Duplicate(m.nsid));
            }
        }
        Ok(Self { methods: out })
    }

    /// Decides whether `caller` may call `nsid` as `kind`. Unknown methods,
    /// the wrong HTTP verb and insufficient credentials are all refused.
    pub fn authorize(&self, nsid: &str, kind: Kind, caller: &Caller) -> Result<&Method, XrpcError> {
        Self::authorize_method(self.resolve(nsid, kind)?, caller)
    }

    fn authorize_method<'a>(method: &'a Method, caller: &Caller) -> Result<&'a Method, XrpcError> {
        let allowed = match (&method.access, caller) {
            (Access::Public, _)
            | (Access::Account, Caller::Account { .. } | Caller::Service { .. })
            | (Access::SpaceCredential, Caller::SpaceCredential { .. })
            | (Access::Session | Access::ServiceOrSession, Caller::Session { .. })
            | (Access::Service | Access::ServiceOrSession, Caller::Service { .. }) => true,
            (
                Access::Account
                | Access::Service
                | Access::ServiceOrSession
                | Access::SpaceCredential
                | Access::Session,
                _,
            ) => false,
        };
        if !allowed {
            return Err(XrpcError::AuthRequired);
        }
        if let Caller::Service { lxm, .. } = caller {
            // A service token is bound to exactly one method.
            if lxm != method.nsid {
                return Err(XrpcError::AuthRequired);
            }
        }
        Ok(method)
    }

    /// Resolves a declaration and verb before any credential work begins.
    fn resolve(&self, nsid: &str, kind: Kind) -> Result<&Method, XrpcError> {
        let method = self.methods.get(nsid).ok_or(XrpcError::MethodNotImplemented)?;
        if method.kind != kind {
            return Err(XrpcError::InvalidRequest(
                "wrong HTTP method for this XRPC method".into(),
            ));
        }
        Ok(method)
    }

    pub fn methods(&self) -> impl Iterator<Item = &Method> {
        self.methods.values()
    }
}

/// Who is calling, as established by the transport before any handler runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    Anonymous,
    /// A Motet browser session bound to this account.
    Account {
        did: String,
    },
    /// A verified OAuth session acting as a World, with its human actor and
    /// client retained for current-role and scope checks at each write.
    Session {
        world: String,
        actor: String,
        client_id: String,
        scopes: Vec<String>,
    },
    /// A verified service-auth token: issuer, the one method it allows, and
    /// the audience it was minted for. A tenor answers for many Worlds, so
    /// handlers check that the World a request names is the audience.
    Service {
        did: String,
        lxm: String,
        audience: String,
    },
    /// A verified Space credential, not an account session. Its subject is
    /// a Space, so it must never inherit account or service privileges.
    SpaceCredential {
        space: String,
        audience: String,
    },
}

impl Caller {
    #[must_use]
    pub fn did(&self) -> Option<&str> {
        match self {
            Self::Anonymous | Self::SpaceCredential { .. } => None,
            Self::Account { did } | Self::Service { did, .. } => Some(did),
            Self::Session { world, .. } => Some(world),
        }
    }
}

/// atproto's error shape: `{"error": "Name", "message": "..."}`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum XrpcError {
    #[error("method not implemented")]
    MethodNotImplemented,
    #[error("authentication required")]
    AuthRequired,
    /// The verifier requires a fresh `DPoP` proof carrying this server nonce.
    #[error("a fresh DPoP nonce is required")]
    DpopNonceRequired { nonce: String },
    #[error("{0}")]
    InvalidRequest(String),
    #[error("not found")]
    NotFound,
    #[error("the request body is larger than {0} bytes")]
    PayloadTooLarge(usize),
    /// A protocol-defined refusal whose name and status a client relies on.
    #[error("{message}")]
    Protocol {
        status: StatusCode,
        name: &'static str,
        message: String,
    },
    #[error("internal error")]
    Internal,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: String,
}

impl IntoResponse for XrpcError {
    fn into_response(self) -> Response {
        let (status, name) = match &self {
            Self::MethodNotImplemented => (StatusCode::NOT_IMPLEMENTED, "MethodNotImplemented"),
            Self::AuthRequired => (StatusCode::UNAUTHORIZED, "AuthRequired"),
            Self::DpopNonceRequired { .. } => (StatusCode::UNAUTHORIZED, "use_dpop_nonce"),
            Self::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "InvalidRequest"),
            Self::NotFound => (StatusCode::NOT_FOUND, "NotFound"),
            Self::PayloadTooLarge(_) => (StatusCode::PAYLOAD_TOO_LARGE, "PayloadTooLarge"),
            Self::Protocol { status, name, .. } => (*status, *name),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError"),
        };
        let nonce = if let Self::DpopNonceRequired { nonce } = &self {
            let Ok(nonce) = HeaderValue::from_str(nonce) else {
                return Self::Internal.into_response();
            };
            Some(nonce)
        } else {
            None
        };
        let mut response = (
            status,
            Json(ErrorBody {
                error: name,
                message: self.to_string(),
            }),
        )
            .into_response();
        if let Some(nonce) = nonce {
            response.headers_mut().insert("dpop-nonce", nonce);
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("DPoP error=\"use_dpop_nonce\""),
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn registry() -> Registry {
        Registry::new([
            Method {
                nsid: "at.motet.pilot.getScene",
                kind: Kind::Query,
                access: Access::Public,
                max_body: 0,
            },
            Method {
                nsid: "at.motet.pilot.submitPost",
                kind: Kind::Procedure,
                access: Access::Account,
                max_body: 1 << 16,
            },
            Method {
                nsid: "at.motet.pilot.play.submitAdmission",
                kind: Kind::Procedure,
                access: Access::Service,
                max_body: 1 << 12,
            },
            Method {
                nsid: "test.session",
                kind: Kind::Procedure,
                access: Access::Session,
                max_body: 0,
            },
            Method {
                nsid: "test.serviceOrSession",
                kind: Kind::Procedure,
                access: Access::ServiceOrSession,
                max_body: 0,
            },
        ])
        .expect("no duplicates")
    }

    fn caller() -> impl Strategy<Value = Caller> {
        let nsid = prop_oneof![
            Just("at.motet.pilot.getScene".to_owned()),
            Just("at.motet.pilot.submitPost".to_owned()),
            Just("at.motet.pilot.play.submitAdmission".to_owned()),
            Just("test.session".to_owned()),
            Just("test.serviceOrSession".to_owned()),
            "[a-z.]{1,30}",
        ];
        prop_oneof![
            Just(Caller::Anonymous),
            Just(Caller::Account {
                did: "did:plc:a".into()
            }),
            Just(Caller::Session {
                world: "did:plc:world".into(),
                actor: "did:plc:a".into(),
                client_id: "https://client.test/metadata.json".into(),
                scopes: vec!["atproto".into()],
            }),
            Just(Caller::SpaceCredential {
                space: "at://did:plc:world/space/group.opensocial.meta/self".into(),
                audience: "did:plc:world".into(),
            }),
            nsid.prop_map(|lxm| Caller::Service {
                did: "did:plc:a".into(),
                lxm,
                audience: "did:web:motet.test".into(),
            }),
        ]
    }

    proptest! {
        /// Fail closed: whatever the caller, an undeclared method is never reached.
        #[test]
        fn undeclared_methods_are_never_reachable(nsid in "[a-zA-Z.]{1,40}", caller in caller()) {
            let r = registry();
            prop_assume!(r.methods().all(|m| m.nsid != nsid));
            prop_assert_eq!(r.authorize(&nsid, Kind::Query, &caller).err(), Some(XrpcError::MethodNotImplemented));
            prop_assert_eq!(r.authorize(&nsid, Kind::Procedure, &caller).err(), Some(XrpcError::MethodNotImplemented));
        }

        /// A service token opens exactly the one method it names.
        #[test]
        fn service_tokens_open_only_their_own_method(caller in caller()) {
            let r = registry();
            if let Caller::Service { lxm, .. } = &caller {
                for m in r.methods().filter(|m| matches!(m.access, Access::Account | Access::Service | Access::ServiceOrSession)) {
                    let ok = r.authorize(m.nsid, m.kind, &caller).is_ok();
                    prop_assert_eq!(ok, lxm == m.nsid, "{} with token for {}", m.nsid, lxm);
                }
            }
        }

        /// Only World sessions reach session methods. Reusing the Account rule
        /// here would admit browser accounts or a service token with this lxm.
        #[test]
        fn session_methods_exclude_every_other_credential(caller in caller()) {
            let registry = registry();
            let result = registry.authorize("test.session", Kind::Procedure, &caller);
            if matches!(caller, Caller::Session { .. }) {
                prop_assert!(result.is_ok());
            } else {
                prop_assert_eq!(result.err(), Some(XrpcError::AuthRequired));
            }
        }
    }

    #[test]
    fn duplicate_declarations_are_rejected() {
        let m = Method {
            nsid: "x.y.z",
            kind: Kind::Query,
            access: Access::Public,
            max_body: 0,
        };
        assert_eq!(
            Registry::new([m.clone(), m]).err(),
            Some(RegistryError::Duplicate("x.y.z"))
        );
    }
}
