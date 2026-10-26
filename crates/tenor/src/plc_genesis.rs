//! Genesis shape and rotation-key priority ported from atproto-pds
//! `src/plc.rs::{prepare_genesis, build_genesis_operation}` at 09a868cd
//! (MIT). Local changes are recorded in `crates/tenor/UPSTREAM.md`.

use atproto_identity::errors::PLCDIDError;
use atproto_identity::key::KeyData;
use atproto_identity::plc::{Operation, PlcState, ServiceEndpoint, UnsignedOperation, UpdateBuilder};

/// The owner outranks the optional tenor rotation key. Genesis is permanent:
/// putting the tenor first would let it defeat the owner's recovery.
pub(crate) fn prepare(
    handle: &str,
    endpoint: &str,
    owner: KeyData,
    signing: KeyData,
    kem: KeyData,
    rotation: Option<KeyData>,
) -> Result<UnsignedOperation, PLCDIDError> {
    let mut rotations = Vec::with_capacity(1 + usize::from(rotation.is_some()));
    rotations.push(owner);
    rotations.extend(rotation);
    // DidBuilder always signs and therefore needs a private rotation key.
    // UpdateBuilder validates the same public state without asking for one;
    // only that state is used to construct the unsigned genesis below.
    let (state, _) = UpdateBuilder::from_state(PlcState::new(), String::new())
        .rotation_keys(rotations)
        .set_verification_method("atproto", signing)
        .set_verification_method("atproto_kem", kem)
        .also_known_as(vec![format!("at://{handle}")])
        .set_service(
            "atproto_pds",
            ServiceEndpoint::new("AtprotoPersonalDataServer".into(), endpoint.into()),
        )
        .set_service(
            "motet_tenor",
            ServiceEndpoint::new("MotetTenor".into(), endpoint.into()),
        )
        .build()?;
    Ok(Operation::new_genesis(
        state.rotation_keys,
        state.verification_methods,
        state.also_known_as,
        state.services,
    ))
}
