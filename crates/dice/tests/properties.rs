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

fn comparison() -> impl Strategy<Value = Comparison> {
    (
        prop_oneof![
            Just(CmpOp::Ge),
            Just(CmpOp::Le),
            Just(CmpOp::Gt),
            Just(CmpOp::Lt),
            Just(CmpOp::Eq),
        ],
        prop_oneof![Just(i64::MIN), -2..=4i64, Just(i64::MAX)],
    )
        .prop_map(|(op, target)| Comparison { op, target })
}

fn finite_dice() -> impl Strategy<Value = Dice> {
    (
        1..=2u32,
        prop_oneof![(1..=3u32).prop_map(Die::Sides), Just(Die::Fudge)],
        proptest::option::of(prop_oneof![
            (0..=3u32).prop_map(Keep::Highest),
            (0..=3u32).prop_map(Keep::Lowest),
            (0..=3u32).prop_map(Keep::DropHighest),
            (0..=3u32).prop_map(Keep::DropLowest),
        ]),
        proptest::option::of(comparison()),
    )
        .prop_map(|(count, die, keep, success)| Dice {
            count,
            die,
            explode: false,
            keep,
            success,
        })
}

fn finite_expr() -> impl Strategy<Value = Expr> {
    let term = || {
        (finite_dice(), 0..=4u32, any::<bool>()).prop_map(|(d, offset, negative)| {
            let shifted = Expr::Bin(Box::new(Expr::Dice(d)), BinOp::Sub, Box::new(Expr::Const(offset)));
            if negative {
                Expr::Neg(Box::new(shifted))
            } else {
                shifted
            }
        })
    };
    let binary = (
        term(),
        prop_oneof![Just(BinOp::Add), Just(BinOp::Sub), Just(BinOp::Mul)],
        term(),
    )
        .prop_map(|(left, op, right)| Expr::Bin(Box::new(left), op, Box::new(right)));
    prop_oneof![
        binary,
        finite_dice().prop_map(|dice| Expr::Dice(Dice {
            count: 1,
            die: Die::Percent,
            ..dice
        })),
    ]
}

/// Remove one extreme at a time, independently of the roller's sorted ranks.
fn finite_dice_total(dice: &Dice, raw: &[u32]) -> i64 {
    let mut faces: Vec<i64> = raw
        .iter()
        .map(|&face| match dice.die {
            Die::Fudge => i64::from(face) - 2,
            Die::Sides(_) | Die::Percent => i64::from(face),
        })
        .collect();
    let (remove, highest) = match dice.keep {
        None => (0, false),
        Some(Keep::Highest(n)) => (dice.count.saturating_sub(n), false),
        Some(Keep::Lowest(n)) => (dice.count.saturating_sub(n), true),
        Some(Keep::DropHighest(n)) => (n.min(dice.count), true),
        Some(Keep::DropLowest(n)) => (n.min(dice.count), false),
    };
    for _ in 0..remove {
        let extreme = if highest {
            faces.iter().max()
        } else {
            faces.iter().min()
        };
        let extreme = *extreme.expect("cannot remove more faces than the dice count");
        let index = faces
            .iter()
            .position(|&face| face == extreme)
            .expect("the extreme is a face");
        faces.remove(index);
    }
    faces
        .into_iter()
        .map(|face| {
            dice.success.map_or(face, |comparison| {
                let ordering = face.cmp(&comparison.target);
                let succeeds = match comparison.op {
                    CmpOp::Ge => !ordering.is_lt(),
                    CmpOp::Le => !ordering.is_gt(),
                    CmpOp::Gt => ordering.is_gt(),
                    CmpOp::Lt => ordering.is_lt(),
                    CmpOp::Eq => ordering.is_eq(),
                };
                i64::from(succeeds)
            })
        })
        .sum()
}

/// Enumerate face assignments, not intervals. Four ternary dice give at most
/// 81 assignments; the separate single percentile die gives 100.
fn finite_outcomes(expr: &Expr) -> Vec<(Vec<u32>, i64)> {
    match expr {
        Expr::Const(value) => vec![(Vec::new(), i64::from(*value))],
        Expr::Dice(dice) => {
            let sides = match dice.die {
                Die::Sides(sides) => sides,
                Die::Fudge => 3,
                Die::Percent => 100,
            };
            let mut scripts = vec![Vec::new()];
            for _ in 0..dice.count {
                scripts = scripts
                    .into_iter()
                    .flat_map(|script| {
                        (1..=sides).map(move |face| {
                            let mut next = script.clone();
                            next.push(face);
                            next
                        })
                    })
                    .collect();
            }
            scripts
                .into_iter()
                .map(|script| {
                    let total = finite_dice_total(dice, &script);
                    (script, total)
                })
                .collect()
        }
        Expr::Neg(inner) => finite_outcomes(inner)
            .into_iter()
            .map(|(script, total)| (script, -total))
            .collect(),
        Expr::Bin(left, op, right) => {
            let right = finite_outcomes(right);
            finite_outcomes(left)
                .into_iter()
                .flat_map(|(left_script, left_total)| {
                    right.iter().map(move |(right_script, right_total)| {
                        let mut script = left_script.clone();
                        script.extend_from_slice(right_script);
                        let total = match op {
                            BinOp::Add => left_total + right_total,
                            BinOp::Sub => left_total - right_total,
                            BinOp::Mul => left_total * right_total,
                        };
                        (script, total)
                    })
                })
                .collect()
        }
    }
}

fn boundary_integer() -> impl Strategy<Value = i64> {
    prop_oneof![
        Just(i64::MIN),
        Just(i64::MIN + 1),
        Just(-i64::from(u32::MAX)),
        Just(-1),
        Just(0),
        Just(1),
        Just(i64::from(u32::MAX)),
        Just(i64::MAX - 1),
        Just(i64::MAX),
        any::<i64>(),
    ]
}

/// Build any signed operand from public u32 constants without overflowing an
/// intermediate. Negative high words let this also represent `i64::MIN`.
fn integer_expr(value: i64) -> Expr {
    let magnitude = value.unsigned_abs();
    let high = u32::try_from(magnitude >> 32).expect("an i64 magnitude has a u32 high word");
    let low = u32::try_from(magnitude & u64::from(u32::MAX)).expect("masked to a u32");
    let high = if value < 0 {
        Expr::Neg(Box::new(Expr::Const(high)))
    } else {
        Expr::Const(high)
    };
    let base = Expr::Bin(Box::new(Expr::Const(u32::MAX)), BinOp::Add, Box::new(Expr::Const(1)));
    Expr::Bin(
        Box::new(Expr::Bin(Box::new(high), BinOp::Mul, Box::new(base))),
        if value < 0 { BinOp::Sub } else { BinOp::Add },
        Box::new(Expr::Const(low)),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every finite face assignment has the mathematical total and its extrema
    /// are attainable. Catches loose bounds, sign errors, and counting dropped successes.
    #[test]
    fn finite_rolls_and_extrema_match_exhaustive_outcomes(expr in finite_expr()) {
        let outcomes = finite_outcomes(&expr);
        let minimum = outcomes.iter().map(|(_, total)| i128::from(*total)).min();
        let maximum = outcomes.iter().map(|(_, total)| i128::from(*total)).max();
        let bounds = expr.bounds();
        prop_assert_eq!((bounds.min, bounds.max), (minimum, maximum), "{}", expr);
        for (script, expected) in outcomes {
            let mut source = Faces::new(&script);
            let roll = expr.roll(&mut source, Limits::default()).expect("small finite expressions fit the roll limits");
            prop_assert_eq!(roll.total, expected, "{}, faces {:?}", expr, script);
            prop_assert!(source.0.is_empty(), "roll did not consume its face assignment");
        }
    }

    /// Direct AST counts respect caller budgets before reading a face.
    /// Catches zero becoming an empty success and u32::MAX wrapping a count check.
    #[test]
    fn public_dice_count_boundaries_refuse_unrollable_requests(
        (count, max_values) in prop_oneof![
            Just((0, 0)),
            Just((0, u32::MAX)),
            Just((1, 0)),
            Just((1, 1)),
            Just((1, u32::MAX)),
            Just((u32::MAX, 0)),
            Just((u32::MAX, u32::MAX - 1)),
            (1..=8u32, 0..=8u32),
        ],
    ) {
        let expr = Expr::Dice(Dice { count, die: Die::Sides(1), explode: false, keep: None, success: None });
        let expected = if count == 0 {
            Err(RollError::Empty)
        } else if count > max_values {
            Err(RollError::TooManyValues(max_values))
        } else {
            Ok(i64::from(count))
        };
        let script = if expected.is_ok() { vec![1; usize::try_from(count).unwrap()] } else { Vec::new() };
        let actual = expr.roll(&mut Faces::new(&script), Limits { max_values, max_sides: 1 });
        prop_assert_eq!(actual.map(|roll| roll.total), expected);
    }

    /// Legal maximum-width dice still return their full face; zero and sides
    /// beyond a caller's limit fail. Catches truncation and an exclusive side cap.
    #[test]
    fn public_dice_side_boundaries_preserve_faces_and_refuse_invalid_sides(
        sides in prop_oneof![Just(0), Just(1), Just(u32::MAX - 1), Just(u32::MAX), any::<u32>()],
        max_sides in prop_oneof![Just(0), Just(1), Just(u32::MAX - 1), Just(u32::MAX)],
        maximum_face in any::<bool>(),
    ) {
        let expr = Expr::Dice(Dice { count: 1, die: Die::Sides(sides), explode: false, keep: None, success: None });
        let face = if maximum_face { sides } else { 1 };
        let expected = if sides == 0 {
            Err(RollError::Empty)
        } else if sides > max_sides {
            Err(RollError::TooManySides(max_sides))
        } else {
            Ok(i64::from(face))
        };
        let script = if expected.is_ok() { vec![face] } else { Vec::new() };
        let actual = expr.roll(&mut Faces::new(&script), Limits { max_values: 1, max_sides });
        prop_assert_eq!(actual.map(|roll| roll.total), expected);
    }

    /// Zero and maximum keep/drop operands saturate over the actual dice.
    /// Catches unsigned subtraction underflow and treating keep-zero as no modifier.
    #[test]
    fn public_keep_boundaries_match_extreme_removal(
        raw in proptest::collection::vec(prop_oneof![Just(1), Just(u32::MAX)], 1..=8),
        boundary in 0..6u8,
        kind in 0..4u8,
    ) {
        let count = u32::try_from(raw.len()).unwrap();
        let amount = match boundary {
            0 => 0,
            1 => 1,
            2 => count - 1,
            3 => count,
            4 => count + 1,
            _ => u32::MAX,
        };
        let keep = match kind {
            0 => Keep::Highest(amount),
            1 => Keep::Lowest(amount),
            2 => Keep::DropHighest(amount),
            _ => Keep::DropLowest(amount),
        };
        let dice = Dice {
            count,
            die: Die::Sides(u32::MAX),
            explode: false,
            keep: Some(keep),
            success: None,
        };
        let expected = finite_dice_total(&dice, &raw);
        let expr = Expr::Dice(dice);
        let actual = expr
            .roll(&mut Faces::new(&raw), Limits { max_values: 8, max_sides: u32::MAX })
            .expect("eight finite dice fit the explicit limits");
        prop_assert_eq!(actual.total, expected);
    }

    /// Public arithmetic returns the exact mathematical integer or refuses
    /// overflow. Catches wrapping, saturating arithmetic and wrong signed endpoints.
    #[test]
    fn public_arithmetic_boundaries_match_wide_integer_results(
        left in boundary_integer(),
        right in boundary_integer(),
        op in prop_oneof![Just(BinOp::Add), Just(BinOp::Sub), Just(BinOp::Mul)],
    ) {
        let expected = match op {
            BinOp::Add => i128::from(left) + i128::from(right),
            BinOp::Sub => i128::from(left) - i128::from(right),
            BinOp::Mul => i128::from(left) * i128::from(right),
        };
        let expr = Expr::Bin(Box::new(integer_expr(left)), op, Box::new(integer_expr(right)));
        let actual = expr.roll(&mut Faces::new(&[]), Limits { max_values: 0, max_sides: 0 });
        prop_assert_eq!(
            actual.map(|roll| roll.total),
            i64::try_from(expected).map_err(|_| RollError::Overflow),
        );
        let bounds = expr.bounds();
        prop_assert_eq!((bounds.min, bounds.max), (Some(expected), Some(expected)));
    }

    /// Negation refuses only the unrepresentable positive counterpart of MIN.
    /// Catches unchecked negation panics and saturation at the signed boundary.
    #[test]
    fn public_negation_boundaries_match_wide_integer_results(value in boundary_integer()) {
        let expr = Expr::Neg(Box::new(integer_expr(value)));
        let expected = -i128::from(value);
        let actual = expr.roll(&mut Faces::new(&[]), Limits { max_values: 0, max_sides: 0 });
        prop_assert_eq!(
            actual.map(|roll| roll.total),
            i64::try_from(expected).map_err(|_| RollError::Overflow),
        );
        let bounds = expr.bounds();
        prop_assert_eq!((bounds.min, bounds.max), (Some(expected), Some(expected)));
    }

    /// A later zero factor cannot turn an overflowing subexpression into a
    /// successful roll. Catches evaluating only the final mathematical result.
    #[test]
    fn intermediate_overflow_cannot_be_cancelled(
        negative in any::<bool>(),
        excess in 1..=u32::MAX,
    ) {
        let (endpoint, op) = if negative { (i64::MIN, BinOp::Sub) } else { (i64::MAX, BinOp::Add) };
        let overflowing = Expr::Bin(Box::new(integer_expr(endpoint)), op, Box::new(Expr::Const(excess)));
        let expr = Expr::Bin(Box::new(overflowing), BinOp::Mul, Box::new(Expr::Const(0)));
        prop_assert_eq!(
            expr.roll(&mut Faces::new(&[]), Limits::default()).map(|roll| roll.total),
            Err(RollError::Overflow),
        );
    }
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
    // An exploding factor has no maximum, so the product has none. Its minimum
    // may be loose; totals_stay_within_bounds checks that it is sound.
    assert_eq!(bounds("1d6!*2").1, None);

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
    // A run of minus signs nests too, and meets the same limit.
    let negated = |n: usize| format!("{}1", "-".repeat(n));
    assert!(parse(&negated(32)).is_ok());
    assert_eq!(parse(&negated(33)), Err(ParseError::TooDeep));
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
    // At the end of the input no byte is at fault, so this checks only the reason.
    assert!(
        matches!(parse("2d6+"), Err(ParseError::Unexpected { found, .. }) if found == "end of input"),
        "a dangling operator is an error"
    );
}
