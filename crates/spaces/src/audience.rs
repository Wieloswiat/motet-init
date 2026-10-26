//! Which Space holds which audience.
//!
//! A Space can't hide records from some of its readers, and opensocial.group
//! grants access by role, so each audience that must be kept apart gets a
//! Space of its own. Records in different Spaces can share author,
//! collection and key, so every reference to private content is Space-aware.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A Space: its authority (the World's DID), its type and its key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SpaceId {
    pub authority: String,
    pub kind: String,
    pub key: String,
}

impl SpaceId {
    /// The protocol Space reference: `at://<authority>/space/<type>/<skey>`.
    ///
    /// The caller supplies a valid authority DID, Space-type NSID and record key.
    /// Validation stays at the protocol boundary, outside this seam.
    #[must_use]
    pub fn space_ref(&self) -> String {
        format!("at://{}/space/{}/{}", self.authority, self.kind, self.key)
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.authority, self.kind, self.key)
    }
}

/// An audience within a World.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Audience {
    /// opensocial.group's `meta` Space: profile and rules.
    Meta,
    /// opensocial.group's `members` Space: roles, permissions, membership.
    Members,
    /// GM notes, reasons for decisions, reviewers' discussion.
    Gms,
    /// One private scene's cast.
    SceneCast { scene: String },
    /// One application under review: the applicant and the reviewers.
    Application { application: String },
}

/// Space types. opensocial.group's are pinned to the revision Motet adopts;
/// Motet's own are drafts in the pilot namespace.
pub mod kinds {
    pub const META: &str = "group.opensocial.meta";
    pub const MEMBERS: &str = "group.opensocial.members";
    /// A user's inbox, not an audience under the World's authority.
    pub const INVITES: &str = "group.opensocial.invites";
    pub const GMS: &str = "at.motet.pilot.space.gms";
    pub const SCENE: &str = "at.motet.pilot.space.scene";
    pub const APPLICATION: &str = "at.motet.pilot.space.application";
}

impl Audience {
    /// The Space that holds this audience in `world`.
    #[must_use]
    pub fn space(&self, world: &str) -> SpaceId {
        let (kind, key) = match self {
            Self::Meta => (kinds::META, "self"),
            Self::Members => (kinds::MEMBERS, "self"),
            Self::Gms => (kinds::GMS, "self"),
            Self::SceneCast { scene } => (kinds::SCENE, scene.as_str()),
            Self::Application { application } => (kinds::APPLICATION, application.as_str()),
        };
        SpaceId {
            authority: world.to_owned(),
            kind: kind.to_owned(),
            key: key.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn audience() -> impl Strategy<Value = Audience> {
        prop_oneof![
            Just(Audience::Meta),
            Just(Audience::Members),
            Just(Audience::Gms),
            "[a-z0-9]{1,8}".prop_map(|scene| Audience::SceneCast { scene }),
            "[a-z0-9]{1,8}".prop_map(|application| Audience::Application { application }),
        ]
    }

    proptest! {
        /// Two different audiences never share a Space, in any World, and no
        /// two Worlds share one: sharing would let one audience read the
        /// other's records with its credential. Each Space also prints
        /// differently, so a sync error names exactly one Space.
        #[test]
        fn audiences_never_share_a_space(a in audience(), b in audience(), wa in "did:plc:[a-z]{1,4}", wb in "did:plc:[a-z]{1,4}") {
            let same = a == b && wa == wb;
            prop_assert_eq!(a.space(&wa) == b.space(&wb), same);
            prop_assert_eq!(a.space(&wa).to_string() == b.space(&wb).to_string(), same);
            prop_assert_eq!(a.space(&wa).space_ref() == b.space(&wb).space_ref(), same);
        }
    }

    /// The protocol marker and adopted type must reach callers unchanged.
    /// Omitting `space` or using the old `.space.meta` NSID misaddresses the Space.
    #[test]
    fn space_reference_addresses_the_adopted_meta_space() {
        assert_eq!(
            Audience::Meta.space("did:plc:world").space_ref(),
            "at://did:plc:world/space/group.opensocial.meta/self"
        );
    }
}
