//! Sealed World keys open only for the World and purpose they were sealed
//! for, and only under the master key that sealed them. A sealed blob copied
//! into another World's slot in the bucket, or read with the wrong purpose,
//! must not yield a key: that would let one World sign as another.

use atproto_identity::key::to_public;
use motet_rules::Did;
use motet_tenor::keys::{KeyError, MasterKey, Purpose, generate};

const PURPOSES: [Purpose; 3] = [Purpose::Signing, Purpose::Kem, Purpose::Rotation];

fn public(key: &atproto_identity::key::KeyData) -> String {
    to_public(key).expect("public half").to_string()
}

#[test]
fn a_sealed_key_opens_only_where_it_was_sealed() {
    let master = MasterKey::generate().expect("master key");
    let other_master = MasterKey::generate().expect("master key");
    let (world, other_world) = (Did::new("did:plc:world-a"), Did::new("did:plc:world-b"));

    for purpose in PURPOSES {
        let key = generate(purpose).expect("key");
        let sealed = master.seal(&world, purpose, &key).expect("seal");

        let opened = master.unseal(&world, purpose, &sealed).expect("opens where sealed");
        assert_eq!(public(&opened), public(&key), "{purpose:?} round-trips");

        assert!(matches!(
            master.unseal(&other_world, purpose, &sealed),
            Err(KeyError::Unseal)
        ));
        for other_purpose in PURPOSES.into_iter().filter(|p| *p != purpose) {
            assert!(
                matches!(master.unseal(&world, other_purpose, &sealed), Err(KeyError::Unseal)),
                "{purpose:?} sealed, opened as {other_purpose:?}"
            );
        }
        assert!(matches!(
            other_master.unseal(&world, purpose, &sealed),
            Err(KeyError::Unseal)
        ));
    }
}

#[test]
fn a_tampered_sealed_key_does_not_open() {
    let master = MasterKey::generate().expect("master key");
    let world = Did::new("did:plc:world-a");
    let sealed = master
        .seal(&world, Purpose::Signing, &generate(Purpose::Signing).expect("key"))
        .expect("seal");
    for i in 1..sealed.len() {
        let mut tampered = sealed.clone();
        if let Some(byte) = tampered.get_mut(i) {
            *byte ^= 1;
        }
        assert!(
            master.unseal(&world, Purpose::Signing, &tampered).is_err(),
            "flipping byte {i} still opened"
        );
    }
}
