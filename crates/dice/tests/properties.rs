//! Properties of dice notation and rolling.

use motet_dice::{
    BinOp, CmpOp, Comparison, Dice, Die, Expr, Keep, Limits, MAX_LEN, ParseError, RollError, Source, parse,
};
use proptest::prelude::*;

/// Faces from a proptest-generated script, wrapped into range.
struct Scripted {
    faces: Vec<u32>,
    next: usize,
}

impl Source for Scripted {
    fn roll(&mut self, sides: u32) -> u32 {
        let raw = self
            .faces
            .get(self.next % self.faces.len().max(1))
            .copied()
            .unwrap_or(0);
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
    let cmp = prop_oneof![
        Just(CmpOp::Ge),
        Just(CmpOp::Le),
        Just(CmpOp::Gt),
        Just(CmpOp::Lt),
        Just(CmpOp::Eq)
    ];
    (
        1..8u32,
        die(),
        any::<bool>(),
        proptest::option::of(keep),
        proptest::option::of((cmp, -2..10i64)),
    )
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
            (
                inner.clone(),
                prop_oneof![Just(BinOp::Add), Just(BinOp::Sub), Just(BinOp::Mul)],
                inner
            )
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
        assert!(
            matches!(e.roll(&mut Max, Limits::default()), Err(RollError::Explode(_))),
            "{text}"
        );
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

/// Returns the given faces in order, exactly as given.
struct Faces(Vec<u32>);

impl Faces {
    fn new(faces: &[u32]) -> Self {
        let mut faces = faces.to_vec();
        faces.reverse();
        Self(faces)
    }
}

impl Source for Faces {
    fn roll(&mut self, sides: u32) -> u32 {
        let face = self.0.pop().expect("the test scripted enough faces");
        assert!((1..=sides).contains(&face), "scripted face {face} isn't on a d{sides}");
        face
    }
}

fn total(text: &str, faces: &[u32]) -> Result<i64, RollError> {
    let e = parse(text).expect("valid notation");
    e.roll(&mut Faces::new(faces), Limits::default()).map(|r| r.total)
}

/// Each comparison counts a face equal to its target the way its operator
/// says, so swapping `>=` for `>` (or any other pair) changes a result here.
#[test]
fn successes_count_faces_against_the_target() {
    // One face below the target, two on it, four above: every operator gets
    // a different count, so no two can be confused.
    let faces = [3, 8, 8, 9, 10, 9, 10];
    let cases = [("<8", 1), ("=8", 2), (">8", 4), ("<=8", 3), (">=8", 6)];
    for (cmp, successes) in cases {
        assert_eq!(total(&format!("7d10{cmp}"), &faces), Ok(successes), "7d10{cmp}");
    }
}

/// Successes are counted over the dice keep/drop kept, after explosions.
#[test]
fn successes_count_only_kept_dice() {
    // 3d6!: 6 explodes into 2; then keep the highest two of [6, 2, 5, 1].
    assert_eq!(total("3d6!kh2>=5", &[6, 2, 5, 1]), Ok(2));
    assert_eq!(total("3d6!kl2>=5", &[6, 2, 5, 1]), Ok(0));
}

#[test]
fn sides_limit_is_inclusive() {
    let limits = Limits {
        max_sides: 20,
        ..Limits::default()
    };
    let roll = |text: &str| parse(text).expect("valid").roll(&mut Faces::new(&[1]), limits);
    assert!(roll("1d20").is_ok());
    assert_eq!(roll("1d21").err(), Some(RollError::TooManySides(20)));
}

#[test]
fn value_limit_is_inclusive_and_counts_explosions() {
    let limits = Limits {
        max_values: 3,
        ..Limits::default()
    };
    let roll = |text: &str, faces: &[u32]| parse(text).expect("valid").roll(&mut Faces::new(faces), limits);
    assert!(roll("3d6", &[1, 2, 3]).is_ok());
    // One explosion makes a fourth value.
    assert_eq!(roll("3d6!", &[6, 2, 3, 4]).err(), Some(RollError::TooManyValues(3)));
    // Values from every term count towards the same limit.
    assert_eq!(roll("2d6+2d6", &[1, 2, 3, 4]).err(), Some(RollError::TooManyValues(3)));
}

/// An oversized count fails before any die is rolled: `Faces` panics if
/// asked for a face it wasn't given.
#[test]
fn oversized_counts_fail_before_rolling() {
    let e = parse("1001d6").expect("valid");
    assert_eq!(
        e.roll(&mut Faces::new(&[]), Limits::default()).err(),
        Some(RollError::TooManyValues(1000))
    );
}

#[test]
fn empty_dice_are_refused() {
    for text in ["0d6", "1d0"] {
        assert_eq!(total(text, &[]), Err(RollError::Empty), "{text}");
    }
}

#[test]
fn fudge_dice_read_minus_one_to_plus_one() {
    // The source's 1, 2, 3 are the faces -, blank, +.
    assert_eq!(total("4dF", &[1, 2, 3, 3]), Ok(1));
    assert_eq!(total("2dF", &[1, 1]), Ok(-2));
}

#[test]
fn bounds_examples() {
    let bounds = |text: &str| {
        let b = parse(text).expect("valid").bounds();
        (b.min, b.max)
    };
    assert_eq!(bounds("3d6"), (Some(3), Some(18)));
    assert_eq!(bounds("4d6kh3"), (Some(3), Some(18)));
    assert_eq!(bounds("4d6dl3"), (Some(1), Some(6)));
    assert_eq!(bounds("2dF"), (Some(-2), Some(2)));
    assert_eq!(bounds("3d6!"), (Some(3), None));
    assert_eq!(bounds("5d10>=8"), (Some(0), Some(5)));
    assert_eq!(bounds("2d6>=1"), (Some(2), Some(2)));
    assert_eq!(bounds("2d6>6"), (Some(0), Some(0)));
    assert_eq!(bounds("2d6+1"), (Some(3), Some(13)));
    assert_eq!(bounds("1d6*1d4"), (Some(1), Some(24)));
    assert_eq!(bounds("(1d6-4)*(1d6-4)"), (Some(-6), Some(9)));
    assert_eq!(bounds("1d6*(1d4-5)"), (Some(-24), Some(-1)));
    assert_eq!(bounds("-1d6!"), (None, Some(-1)));
    assert_eq!(bounds("1d6!*2"), (None, None));

    let b = parse("2d6+1").expect("valid").bounds();
    assert!(b.contains(3) && b.contains(13));
    assert!(!b.contains(2) && !b.contains(14));
}

#[test]
fn input_limits_are_inclusive() {
    let longest = format!("1{}", " ".repeat(MAX_LEN - 1));
    assert!(parse(&longest).is_ok());
    assert_eq!(parse(&format!("{longest} ")), Err(ParseError::TooLong));

    let nest = |n: usize| format!("{}1{}", "(".repeat(n), ")".repeat(n));
    assert!(parse(&nest(32)).is_ok());
    assert_eq!(parse(&nest(33)), Err(ParseError::TooDeep));
    assert!(parse(&"-".repeat(32)).is_err());
    // Depth is released after each group: many siblings are fine.
    assert!(parse(&vec!["(((1)))"; 30].join("+")).is_ok());

    assert!(parse("4294967295").is_ok());
    assert_eq!(parse("1+4294967296"), Err(ParseError::NumberTooLarge { at: 2 }));
}

#[test]
fn errors_point_at_the_offending_byte_in_the_original_input() {
    let unexpected = |at, found: &str| {
        Err(ParseError::Unexpected {
            at,
            found: found.into(),
        })
    };
    assert_eq!(parse("1+*"), unexpected(2, "'*'"));
    // Positions count the whitespace the parser skipped.
    assert_eq!(parse("2d6  x"), unexpected(5, "'x'"));
    assert_eq!(parse("2d6+"), unexpected(usize::MAX, "end of input"));
}
