//! Private output stays reader-bound even when authority results arrive out of order.

use motet_appview::private_spaces::access::{
    Permit, PrivateResponse, ReaderLease, SpaceGeneration, Verification, VerificationResult,
};
use motet_spaces::{
    Audience,
    serving::{DEFAULT_BOUND_MILLIS, Decision},
};
use proptest::prelude::*;

fn audience(generation: u64, scene: &str) -> SpaceGeneration {
    SpaceGeneration {
        space: Audience::SceneCast {
            scene: scene.to_owned(),
        }
        .space("did:plc:world"),
        generation,
    }
}

fn clock() -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        0..120_005_u64,
        Just(0),
        Just(u64::MAX),
        Just(u64::MAX - 1)
    ]
}

#[derive(Clone, Copy, Debug)]
enum Fact {
    Start(u64),
    Grant { start: usize, expires: u64 },
    Removal,
    Restart,
}

// The oracle retains authority history, not the implementation's lease fields or tokens.
fn expected(history: &[Fact], now: u64) -> Decision {
    let barrier = history
        .iter()
        .rposition(|fact| matches!(fact, Fact::Removal | Fact::Restart));
    let grant = history
        .iter()
        .filter_map(|fact| match *fact {
            Fact::Grant { start, expires } if barrier.is_none_or(|barrier| start > barrier) => {
                let Fact::Start(at) = *history
                    .get(start)
                    .expect("a grant refers to its recorded verification start")
                else {
                    unreachable!()
                };
                Some((at, expires))
            }
            _ => None,
        })
        .fold(None, |latest: Option<(u64, u64)>, candidate| {
            if latest.is_none_or(|(at, _)| candidate.0 > at) {
                Some(candidate)
            } else {
                latest
            }
        });
    match grant {
        None => Decision::Refuse,
        Some((_, expires)) if now >= expires => Decision::Stale,
        Some((at, _))
            if (u128::from(at)..=u128::from(at) + u128::from(DEFAULT_BOUND_MILLIS)).contains(&u128::from(now)) =>
        {
            Decision::Serve
        }
        Some(_) => Decision::Stale,
    }
}

struct Pending {
    owner: usize,
    start: usize,
    check: Verification,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Delayed grants cannot undo removal, restart cannot restore authority, and Bob cannot change Alice.
    /// Catches stale check installation, response-arrival timestamps and cross-reader/generation caches.
    #[test]
    fn authority_history_controls_output_despite_delayed_results(
        readers in ("did:plc:a[a-z]{1,8}", "did:plc:b[a-z]{1,8}"),
        generation in any::<u64>(),
        operations in prop::collection::vec((0_u8..7, 0_usize..4, any::<usize>(), clock(), clock()), 1..160),
    ) {
        let spaces = [audience(generation, "one"), audience(generation.wrapping_add(1), "one")];
        let mut leases = [
            ReaderLease::new(readers.0.clone(), spaces[0].clone()),
            ReaderLease::new(readers.1.clone(), spaces[0].clone()),
            ReaderLease::new(readers.0.clone(), spaces[1].clone()),
            ReaderLease::new(readers.1.clone(), spaces[1].clone()),
        ];
        let mut histories: [Vec<Fact>; 4] = std::array::from_fn(|_| Vec::new());
        let mut pending: Vec<Pending> = Vec::new();
        let mut permits: Vec<(usize, usize, Permit)> = Vec::new();
        for (operation, mut owner, selection, now, expires) in operations {
            let before: Vec<_> = leases.iter().map(|lease| lease.decision(now)).collect();
            match operation {
                0 => {
                    let start = histories[owner].len();
                    histories[owner].push(Fact::Start(now));
                    pending.push(Pending { owner, start, check: leases[owner].start_verification(now) });
                }
                1..=3 if !pending.is_empty() => {
                    let ticket = pending.swap_remove(selection % pending.len());
                    owner = ticket.owner;
                    let result = match operation {
                        1 => {
                            histories[owner].push(Fact::Grant { start: ticket.start, expires });
                            VerificationResult::Granted { expires_at_millis: expires }
                        }
                        2 => {
                            let restart = histories[owner].iter().rposition(|f| matches!(f, Fact::Restart));
                            if restart.is_none_or(|restart| ticket.start > restart) {
                                histories[owner].push(Fact::Removal);
                            }
                            VerificationResult::Denied
                        }
                        _ => VerificationResult::Unavailable,
                    };
                    leases[owner].finish_verification(ticket.check, result);
                }
                4 => {
                    leases[owner].invalidate();
                    histories[owner].push(Fact::Removal);
                }
                5 => {
                    leases[owner].restart();
                    histories[owner].push(Fact::Restart);
                }
                6 => {
                    let reader = if owner % 2 == 0 { &readers.0 } else { &readers.1 };
                    if let Some(permit) = leases[owner].permit(Some(reader), &spaces[owner / 2], true, now) {
                        permits.push((owner, histories[owner].len(), permit));
                    }
                }
                _ => {}
            }
            for (key, issued, permit) in &permits {
                let reader = if key % 2 == 0 { &readers.0 } else { &readers.1 };
                let revoked = histories[*key].iter().enumerate().any(|(index, fact)|
                    index >= *issued && matches!(fact, Fact::Removal | Fact::Restart));
                let bytes = output(&leases[*key], permit, Some(reader), &spaces[key / 2], true, now);
                prop_assert_eq!(bytes.is_some(), !revoked && expected(&histories[*key], now) == Decision::Serve);
            }
            for index in 0..4 {
                prop_assert_eq!(leases[index].decision(now), expected(&histories[index], now));
                if index != owner {
                    prop_assert_eq!(leases[index].decision(now), before[index]);
                }
            }
        }
    }
}

fn grant(lease: &mut ReaderLease, at: u64, expires: u64) {
    let check = lease.start_verification(at);
    lease.finish_verification(
        check,
        VerificationResult::Granted {
            expires_at_millis: expires,
        },
    );
}

fn output(
    lease: &ReaderLease,
    permit: &Permit,
    reader: Option<&str>,
    space: &SpaceGeneration,
    sponsor: bool,
    now: u64,
) -> Option<Vec<u8>> {
    PrivateResponse::new(space.clone(), b"private prose".to_vec()).release(lease, permit, reader, space, sponsor, now)
}

/// Only the exact live reader/audience receives bytes, never a sync sponsor or anonymous caller.
/// Catches audience-blind permits, caller substitution and cached permits bypassing later revocation.
#[test]
fn private_bytes_require_exact_current_reader_and_space() {
    let space = audience(8, "one");
    let mut alice = ReaderLease::new("alice".to_owned(), space.clone());
    let mut bob = ReaderLease::new("bob".to_owned(), space.clone());
    grant(&mut alice, 100, 100_000);
    grant(&mut bob, 100, 100_000);
    let permit = alice.permit(Some("alice"), &space, true, 100).unwrap();
    let bob_permit = bob.permit(Some("bob"), &space, true, 100).unwrap();
    assert_eq!(
        output(&alice, &permit, Some("alice"), &space, true, 100).unwrap(),
        b"private prose"
    );
    for reader in [None, Some("bob"), Some("sync-sponsor")] {
        assert!(output(&alice, &permit, reader, &space, true, 100).is_none());
    }
    assert!(output(&alice, &bob_permit, Some("alice"), &space, true, 100).is_none());
    assert!(output(&bob, &permit, Some("bob"), &space, true, 100).is_none());
    for other in [audience(9, "one"), audience(8, "two")] {
        assert!(output(&alice, &permit, Some("alice"), &other, true, 100).is_none());
        assert!(
            PrivateResponse::new(other, b"another audience".to_vec())
                .release(&alice, &permit, Some("alice"), &space, true, 100)
                .is_none()
        );
    }
    assert!(output(&alice, &permit, Some("alice"), &space, false, 100).is_none());
    assert!(output(&alice, &permit, Some("alice"), &space, true, 99).is_none());
    assert!(output(&alice, &permit, Some("alice"), &space, true, 60_101).is_none());
    alice.invalidate();
    assert!(output(&alice, &permit, Some("alice"), &space, true, 100).is_none());
    assert!(output(&bob, &bob_permit, Some("bob"), &space, true, 100).is_some());
    grant(&mut alice, 101, 102);
    let fresh = alice.permit(Some("alice"), &space, true, 101).unwrap();
    assert!(output(&alice, &permit, Some("alice"), &space, true, 101).is_none());
    assert!(output(&alice, &fresh, Some("alice"), &space, true, 102).is_none());
    alice.restart();
    grant(&mut alice, 101, 103);
    assert!(output(&alice, &fresh, Some("alice"), &space, true, 101).is_none());
}
