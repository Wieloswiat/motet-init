//! Dice notation for Motet.
//!
//! A subset of standard notation to start (backend plan, "Dice"): `NdM`,
//! `d%`, fudge dice `dF`, integer `+ - *` with parentheses, keep and drop
//! highest or lowest (`kh kl dh dl`), exploding on the maximum face (`!`), and
//! successes against an explicit comparison (`>= <= > < =`). Integers only,
//! since atproto records have no floats, and a cap on how many values one
//! request may generate, so no expression can run away.
//!
//! The pieces are separate on purpose:
//!
//! - [`parse`] turns text into an [`Expr`], and [`Expr`]'s `Display` writes
//!   the canonical form back (`parse(e.to_string()) == e`).
//! - [`Expr::bounds`] computes the range a total can fall in without rolling.
//! - [`Expr::roll`] evaluates against a [`Source`] the caller provides.
//!
//! [`GRAMMAR_VERSION`] goes in every roll result, so a later grammar never
//! reinterprets an old roll.

mod eval;
mod expr;
mod parse;

pub use eval::{Group, Limits, Roll, RollError, Source, Value};
pub use expr::{BinOp, Bounds, CmpOp, Comparison, Dice, Die, Expr, Keep};
pub use parse::{MAX_LEN, ParseError, parse};

/// Identifies this grammar and its semantics in roll results.
pub const GRAMMAR_VERSION: &str = "motet-dice/1";
