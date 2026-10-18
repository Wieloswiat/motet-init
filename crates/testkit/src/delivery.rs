//! How the network delivers records, as proptest strategies.
//!
//! atproto sync orders events within a repository, never across them, and
//! delivers at least once. A consumer that is correct only for the order it
//! happened to see in a test is not correct. These strategies generate the
//! orders a consumer must tolerate, and shrink towards the plain sequential
//! one so a failure reports the smallest reordering that breaks it.

use std::fmt::Debug;

use proptest::prelude::*;
use proptest::sample::Index;

/// Every merge of `streams` that keeps each stream's own order.
///
/// Each inner `Vec` is one repository's events in commit order.
pub fn interleavings<E>(streams: Vec<Vec<E>>) -> impl Strategy<Value = Vec<E>>
where
    E: Clone + Debug,
{
    let tags: Vec<usize> = streams
        .iter()
        .enumerate()
        .flat_map(|(i, s)| std::iter::repeat_n(i, s.len()))
        .collect();
    Just(tags).prop_shuffle().prop_map(move |tags| {
        let mut cursors = vec![0; streams.len()];
        tags.into_iter()
            .map(|i| {
                let event = streams[i][cursors[i]].clone();
                cursors[i] += 1;
                event
            })
            .collect()
    })
}

/// `events` with up to `max` earlier events delivered again at later points,
/// as a sync consumer sees after a lost acknowledgement or a reconnect.
pub fn with_redelivery<E>(events: Vec<E>, max: usize) -> impl Strategy<Value = Vec<E>>
where
    E: Clone + Debug,
{
    proptest::collection::vec((any::<Index>(), any::<Index>()), 0..=max).prop_map(move |picks| {
        let mut out = events.clone();
        for (source, offset) in picks {
            if out.is_empty() {
                break;
            }
            let from = source.index(out.len());
            // Insert strictly after the original: a redelivery is never early.
            let at = from + 1 + offset.index(out.len() - from);
            out.insert(at, out[from].clone());
        }
        out
    })
}

/// [`interleavings`] followed by [`with_redelivery`]: what one consumer of
/// these repositories may receive.
pub fn deliveries<E>(streams: Vec<Vec<E>>, max_redeliveries: usize) -> impl Strategy<Value = Vec<E>>
where
    E: Clone + Debug,
{
    interleavings(streams).prop_flat_map(move |events| with_redelivery(events, max_redeliveries))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn streams() -> impl Strategy<Value = Vec<Vec<(usize, usize)>>> {
        proptest::collection::vec(1..6usize, 1..5).prop_map(|lens| {
            lens.into_iter()
                .enumerate()
                .map(|(s, len)| (0..len).map(|i| (s, i)).collect())
                .collect()
        })
    }

    proptest! {
        #[test]
        fn interleavings_keep_every_event_and_each_streams_order(
            (streams, merged) in streams().prop_flat_map(|s| (Just(s.clone()), interleavings(s)))
        ) {
            prop_assert_eq!(merged.len(), streams.iter().map(Vec::len).sum::<usize>());
            for (s, stream) in streams.iter().enumerate() {
                let seen: Vec<_> = merged.iter().filter(|(t, _)| *t == s).copied().collect();
                prop_assert_eq!(&seen, stream);
            }
        }

        #[test]
        fn redelivery_only_repeats_events_after_their_first_delivery(
            (events, delivered) in proptest::collection::vec(0..1000u32, 0..20)
                .prop_flat_map(|e| (Just(e.clone()), with_redelivery(e, 5)))
        ) {
            // Dropping repeats gives back the original sequence.
            let mut firsts = Vec::new();
            let mut remaining = events.clone();
            for e in &delivered {
                if remaining.first() == Some(e) {
                    firsts.push(*e);
                    remaining.remove(0);
                } else {
                    prop_assert!(firsts.contains(e), "{e} delivered before its first delivery");
                }
            }
            prop_assert_eq!(firsts, events);
        }
    }
}
