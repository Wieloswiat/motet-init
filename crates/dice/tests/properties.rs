//! Properties of dice notation and rolling.

use motet_dice::{BinOp, CmpOp, Comparison, Dice, Die, Expr, Keep, Limits, RollError, Source, parse};
use proptest::prelude::*;

/// Faces from a proptest-generated script, wrapped into range.
struct Scripted {
    faces: Vec<u32>,
    next: usize,
}

impl Source for Scripted {
    fn roll(&mut self, sides: u32) -> u32 {
        let raw = self.faces.get(self.next % self.faces.len().max(1)).copied().unwrap_or(0);
        self.next += 1;
        raw % sides + 1
    }
}

fn die() -> impl Strategy<Value = Die> {
    prop_oneof![
        (1..=100u32).prop_map(Die::Sides),
        Just(Die::Sides(6)),
        Just(Die::Sides(20)),
        Just(Die::Percent),
        Just(Die::Fudge),
    ]
}

fn dice() -> impl Strategy<Value = Dice> {
    let keep = prop_oneof![
        (1..5u32).prop_map(Keep::Highest),
        (1..5u32).prop_map(Keep::Lowest),
        (1..5u32).prop_map(Keep::DropHighest),
        (1..5u32).prop_map(Keep::DropLowest),
    ];
    let cmp = prop_oneof![Just(CmpOp::Ge), Just(CmpOp::Le), Just(CmpOp::Gt), Just(CmpOp::Lt), Just(CmpOp::Eq)];
    (1..8u32, die(), any::<bool>(), proptest::option::of(keep), proptest::option::of((cmp, -2..10i64)))
        .prop_map(|(count, die, explode, keep, success)| Dice {
            count,
            die,
            // Explosion that never stops or never starts is a roll error,
            // covered on its own below.
            explode: explode && matches!(die, Die::Sides(2..) | Die::Percent),
            keep,
            success: success.map(|(op, target)| Comparison { op, target }),
        })
}

fn expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![(0..1000u32).prop_map(Expr::Const), dice().prop_map(Expr::Dice)];
    leaf.prop_recursive(4, 24, 2, |inner| {
        prop_oneof![
            inner.clone().prop_map(|e| Expr::Neg(Box::new(e))),
            (inner.clone(), prop_oneof![Just(BinOp::Add), Just(BinOp::Sub), Just(BinOp::Mul)], inner)
                .prop_map(|(l, op, r)| Expr::Bin(Box::new(l), op, Box::new(r))),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// The canonical form parses back to the same expression, so what a roll
    /// result records is exactly what was rolled.
    #[test]
    fn canonical_form_round_trips(e in expr()) {
        let text = e.to_string();
        prop_assert_eq!(parse(&text), Ok(e), "{}", text);
    }

    /// Every total falls inside the statically computed bounds.
    #[test]
    fn totals_stay_within_bounds(e in expr(), faces in proptest::collection::vec(any::<u32>(), 1..64)) {
        let mut source = Scripted { faces, next: 0 };
        match e.roll(&mut source, Limits::default()) {
            Ok(roll) => prop_assert!(e.bounds().contains(roll.total), "{} = {} outside {:?}", e, roll.total, e.bounds()),
            Err(RollError::TooManyValues(_) | RollError::Overflow) => {}
            Err(other) => prop_assert!(false, "unexpected error {other:?} for {e}"),
        }
    }

    /// No roll generates more values than its limit, however hard it explodes.
    #[test]
    fn rolls_never_exceed_their_value_limit(e in expr(), max_values in 1..50u32) {
        // Always rolling the maximum face makes every exploding die explode.
        struct Max;
        impl Source for Max {
            fn roll(&mut self, sides: u32) -> u32 { sides }
        }
        let limits = Limits { max_values, ..Limits::default() };
        if let Ok(roll) = e.roll(&mut Max, limits) {
            let generated: usize = roll.groups.iter().map(|g| g.values.len()).sum();
            prop_assert!(generated <= max_values as usize);
        }
    }

    /// Keep/drop keeps exactly the number of dice it says, and the right ones.
    #[test]
    fn keep_and_drop_keep_the_extreme_dice(
        d in dice().prop_filter("needs keep", |d| d.keep.is_some() && d.success.is_none()),
        faces in proptest::collection::vec(any::<u32>(), 1..64),
    ) {
        let mut source = Scripted { faces, next: 0 };
        let Ok(roll) = Expr::Dice(d.clone()).roll(&mut source, Limits::default()) else { return Ok(()) };
        let values = &roll.groups[0].values;
        let kept: Vec<i64> = values.iter().filter(|v| v.kept).map(|v| v.face).collect();
        let dropped: Vec<i64> = values.iter().filter(|v| !v.kept).map(|v| v.face).collect();
        let n = values.len();
        let expected = match d.keep {
            Some(Keep::Highest(k) | Keep::Lowest(k)) => (k as usize).min(n),
            Some(Keep::DropHighest(k) | Keep::DropLowest(k)) => n.saturating_sub(k as usize),
            None => unreachable!(),
        };
        prop_assert_eq!(kept.len(), expected);
        let highest = matches!(d.keep, Some(Keep::Highest(_) | Keep::DropLowest(_)));
        for k in &kept {
            for x in &dropped {
                let in_order = if highest { k >= x } else { k <= x };
                prop_assert!(in_order, "kept {} but dropped {}", k, x);
            }
        }
    }

    /// Arbitrary input never panics: it parses or fails with a position.
    #[test]
    fn arbitrary_input_parses_or_fails_cleanly(s in "[0-9dDfFkhl%!()+*<>= -]{0,300}") {
        let _ = parse(&s);
    }
}

#[test]
fn dice_that_cannot_stop_exploding_are_refused() {
    struct Max;
    impl Source for Max {
        fn roll(&mut self, sides: u32) -> u32 {
            sides
        }
    }
    for text in ["1d1!", "4dF!"] {
        let e = parse(text).expect("valid notation");
        assert!(matches!(e.roll(&mut Max, Limits::default()), Err(RollError::Explode(_))), "{text}");
    }
}

#[test]
fn notation_examples() {
    let cases = [
        ("d20", "1d20"),
        ("4d6kh3", "4d6kh3"),
        ("2d20kl1 + 5", "2d20kl1+5"),
        ("(1d6+2)*3", "(1d6+2)*3"),
        ("4dF", "4dF"),
        ("d%", "1d%"),
        ("10d10!>=8", "10d10!>=8"),
        ("1-(2-3)", "1-(2-3)"),
        ("--1", "--1"),
    ];
    for (input, canonical) in cases {
        assert_eq!(parse(input).map(|e| e.to_string()).as_deref(), Ok(canonical), "{input}");
    }
}
