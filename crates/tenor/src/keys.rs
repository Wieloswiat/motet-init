//! Custody of World keys: each is sealed under the tenor's master key before
//! it is written anywhere, so the bucket alone never yields a World's keys.
//!
//! The master key lives outside the bucket (a file the operator provides).
//! Losing it loses every World's signing keys, and backing it up is part of
//! backing up the tenor (the plan's "What a tenor keeps"). A sealed key is
//! bound to its World and purpose as associated data, so a sealed blob
//! copied to another World's slot doesn't open there.
//!
//! [`KeyData`] prints its private key through `Display`. Nothing here formats
//! one, and no error carries one.

use std::fmt;
use std::path::Path;

use atproto_identity::key::{KeyData, KeyType, generate_key};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use motet_rules::Did;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const NONCE_LEN: usize = 24;
/// The sealed format this code writes. Opening a format it doesn't know is
/// an error, never a guess.
const SEALED_FORMAT: u8 = 1;

/// What a World key is for. Each World has at most one key per purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purpose {
    /// The `#atproto` key: signs repo commits and service-auth tokens.
    Signing,
    /// The `#atproto_kem` key: Spaces' shared secrets.
    Kem,
    /// A rotation key the tenor holds, only when the owner opted in.
    Rotation,
}

impl Purpose {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signing => "signing",
            Self::Kem => "kem",
            Self::Rotation => "rotation",
        }
    }

    /// The curve each purpose uses: K-256 for signing, as the reference PDSes
    /// do; P-256 for KEM, which Spaces requires, and for rotation.
    #[must_use]
    pub fn key_type(self) -> KeyType {
        match self {
            Self::Signing => KeyType::K256Private,
            Self::Kem | Self::Rotation => KeyType::P256Private,
        }
    }
}

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("the master key file can't be read: {0}")]
    Read(#[source] std::io::Error),
    #[error("the master key must be 32 bytes, base64 or raw")]
    BadMasterKey,
    #[error("a sealed key didn't open: wrong master key, World or purpose, or corrupt")]
    Unseal,
    #[error("a sealed key has format {0}, which this tenor doesn't know")]
    UnknownFormat(u8),
    #[error("generating a key failed")]
    Generate,
    #[error("the OS random source failed")]
    Random,
}

/// The tenor's master key. Its `Debug` doesn't show the key.
pub struct MasterKey {
    cipher: XChaCha20Poly1305,
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(..)")
    }
}

impl MasterKey {
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(bytes.into()),
        }
    }

    /// Reads a master key file: 32 raw bytes, or their standard base64 with
    /// surrounding whitespace.
    pub fn from_file(path: &Path) -> Result<Self, KeyError> {
        use base64::Engine as _;
        let contents = std::fs::read(path).map_err(KeyError::Read)?;
        let bytes = if let Ok(raw) = <[u8; 32]>::try_from(contents.as_slice()) {
            raw
        } else {
            let text = std::str::from_utf8(&contents).map_err(|_| KeyError::BadMasterKey)?;
            base64::engine::general_purpose::STANDARD
                .decode(text.trim())
                .ok()
                .and_then(|decoded| <[u8; 32]>::try_from(decoded).ok())
                .ok_or(KeyError::BadMasterKey)?
        };
        Ok(Self::from_bytes(&bytes))
    }

    /// A fresh random master key, for tests and first-run setup.
    pub fn generate() -> Result<Self, KeyError> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| KeyError::Random)?;
        Ok(Self::from_bytes(&bytes))
    }

    /// Seals a private key for one World and purpose.
    pub fn seal(&self, world: &Did, purpose: Purpose, key: &KeyData) -> Result<Vec<u8>, KeyError> {
        self.seal_for(world.as_str(), purpose, key)
    }

    pub(crate) fn seal_reserved(
        &self,
        reservation: &str,
        purpose: Purpose,
        key: &KeyData,
    ) -> Result<Vec<u8>, KeyError> {
        self.seal_for(&format!("reservation/{reservation}"), purpose, key)
    }

    fn seal_for(&self, subject: &str, purpose: Purpose, key: &KeyData) -> Result<Vec<u8>, KeyError> {
        self.seal_secret(&associated_data(subject, purpose), key.bytes())
    }

    fn seal_secret(&self, context: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, KeyError> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|_| KeyError::Random)?;
        let sealed = self
            .cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: context,
                },
            )
            .map_err(|_| KeyError::Unseal)?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
        out.push(SEALED_FORMAT);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        Ok(out)
    }

    /// Opens a key [`MasterKey::seal`] sealed for the same World and purpose.
    pub fn unseal(&self, world: &Did, purpose: Purpose, sealed: &[u8]) -> Result<KeyData, KeyError> {
        self.unseal_for(world.as_str(), purpose, sealed)
    }

    pub(crate) fn unseal_reserved(
        &self,
        reservation: &str,
        purpose: Purpose,
        sealed: &[u8],
    ) -> Result<KeyData, KeyError> {
        self.unseal_for(&format!("reservation/{reservation}"), purpose, sealed)
    }

    fn unseal_for(&self, subject: &str, purpose: Purpose, sealed: &[u8]) -> Result<KeyData, KeyError> {
        let bytes = self.unseal_secret(&associated_data(subject, purpose), sealed)?;
        Ok(KeyData::new(purpose.key_type(), bytes))
    }

    fn unseal_secret(&self, context: &[u8], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let (&format, rest) = sealed.split_first().ok_or(KeyError::Unseal)?;
        if format != SEALED_FORMAT {
            return Err(KeyError::UnknownFormat(format));
        }
        let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN).ok_or(KeyError::Unseal)?;
        let nonce = <[u8; NONCE_LEN]>::try_from(nonce).map_err(|_| KeyError::Unseal)?;
        let bytes = self
            .cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: ciphertext,
                    aad: context,
                },
            )
            .map_err(|_| KeyError::Unseal)?;
        Ok(bytes)
    }
}

/// A new private key for `purpose`.
pub fn generate(purpose: Purpose) -> Result<KeyData, KeyError> {
    generate_key(purpose.key_type()).map_err(|_| KeyError::Generate)
}

fn associated_data(subject: &str, purpose: Purpose) -> Vec<u8> {
    let mut aad = Vec::with_capacity(subject.len() + 1 + purpose.as_str().len());
    aad.extend_from_slice(subject.as_bytes());
    aad.push(0);
    aad.extend_from_slice(purpose.as_str().as_bytes());
    aad
}
