//! Rolling for real: the operating system's secure random source.
//!
//! Issuing a roll is a World command like any other: one result per request
//! (and per roll opportunity slot), committed to the shard before it is
//! returned. A timeout means pending, never a new roll; a request over the
//! limits ends in a recorded failure. That flow arrives with the roll
//! opportunity records; this module is only where faces come from.

use motet_dice::Source;

/// Faces from the OS's secure random source, uniform over `1..=sides`.
#[derive(Debug, Default)]
pub struct OsSource;

impl Source for OsSource {
    fn roll(&mut self, sides: u32) -> u32 {
        uniform(sides, || getrandom::u32().expect("the OS random source is available"))
    }
}

/// A uniform face in `1..=sides` from uniform 32-bit words, by rejection: a
/// plain `word % sides` would favour low faces whenever `sides` doesn't
/// divide 2³².
pub fn uniform(sides: u32, mut word: impl FnMut() -> u32) -> u32 {
    assert!(sides > 0, "a die has at least one side");
    // Words in `zone..` would make the faces uneven; draw again.
    let zone = u32::MAX - (u32::MAX - sides + 1) % sides;
    loop {
        let w = word();
        if w <= zone {
            return w % sides + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn faces_are_always_on_the_die(sides in 1..=u32::MAX, words in proptest::collection::vec(any::<u32>(), 1..8)) {
            // Zero is never rejected, so the script always ends.
            let mut words = words.into_iter().chain(std::iter::repeat(0));
            let face = uniform(sides, || words.next().unwrap_or(0));
            prop_assert!((1..=sides).contains(&face));
        }
    }

    #[test]
    fn words_past_the_even_zone_are_drawn_again() {
        // For a d3, 2³² ≡ 1 (mod 3): exactly one word, u32::MAX, is rejected.
        let mut words = [u32::MAX, 4].into_iter();
        assert_eq!(uniform(3, || words.next().expect("two words")), 4 % 3 + 1);
        let mut words = [u32::MAX - 1].into_iter();
        assert_eq!(uniform(3, || words.next().expect("one word")), (u32::MAX - 1) % 3 + 1);
    }
}
