use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::expr::{BinOp, Dice, Die, Expr, Keep};

/// Where faces come from. The tenor implements it over the operating system's
/// secure random source; tests script it.
pub trait Source {
    /// A uniformly random integer in `1..=sides`. `sides` is at least 1.
    fn roll(&mut self, sides: u32) -> u32;
}

/// What one request may cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Every value generated counts, explosions included.
    pub max_values: u32,
    pub max_sides: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_values: 1000,
            max_sides: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum RollError {
    #[error("the roll would generate more than {0} values")]
    TooManyValues(u32),
    #[error("dice can have at most {0} sides")]
    TooManySides(u32),
    #[error("a die needs at least one side, and a roll at least one die")]
    Empty,
    #[error("{0} can't explode: it would never stop, or never start")]
    Explode(String),
    #[error("the total doesn't fit in a 64-bit integer")]
    Overflow,
}

/// One rolled value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Value {
    pub face: i64,
    /// Rolled because the previous die exploded.
    pub exploded: bool,
    /// Counted after keep/drop.
    pub kept: bool,
}

/// The values one dice term produced, in the order they were rolled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// The term in canonical notation, e.g. `4d6kh3`.
    pub dice: String,
    pub values: Vec<Value>,
}

/// A completed roll: what gets committed to the World's repo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roll {
    pub grammar: String,
    pub expression: String,
    pub total: i64,
    pub groups: Vec<Group>,
}

impl Expr {
    /// Rolls the expression. Either the whole roll completes or it fails with
    /// a reason: a failed roll is recorded as such, never retried for a
    /// different result.
    pub fn roll(&self, source: &mut dyn Source, limits: Limits) -> Result<Roll, RollError> {
        let mut ctx = Ctx {
            source,
            limits,
            generated: 0,
            groups: Vec::new(),
        };
        let total = ctx.eval(self)?;
        Ok(Roll {
            grammar: crate::GRAMMAR_VERSION.to_owned(),
            expression: self.to_string(),
            total,
            groups: ctx.groups,
        })
    }
}

struct Ctx<'a> {
    source: &'a mut dyn Source,
    limits: Limits,
    generated: u32,
    groups: Vec<Group>,
}

impl Ctx<'_> {
    fn eval(&mut self, e: &Expr) -> Result<i64, RollError> {
        match e {
            Expr::Const(n) => Ok(i64::from(*n)),
            Expr::Neg(e) => self.eval(e)?.checked_neg().ok_or(RollError::Overflow),
            Expr::Bin(l, op, r) => {
                let (l, r) = (self.eval(l)?, self.eval(r)?);
                match op {
                    BinOp::Add => l.checked_add(r),
                    BinOp::Sub => l.checked_sub(r),
                    BinOp::Mul => l.checked_mul(r),
                }
                .ok_or(RollError::Overflow)
            }
            Expr::Dice(d) => self.dice(d),
        }
    }

    fn face(&mut self, die: Die) -> Result<i64, RollError> {
        self.generated += 1;
        if self.generated > self.limits.max_values {
            return Err(RollError::TooManyValues(self.limits.max_values));
        }
        Ok(match die {
            Die::Sides(n) => i64::from(self.source.roll(n)),
            Die::Percent => i64::from(self.source.roll(100)),
            Die::Fudge => i64::from(self.source.roll(3)) - 2,
        })
    }

    fn dice(&mut self, d: &Dice) -> Result<i64, RollError> {
        match d.die {
            Die::Sides(0) => return Err(RollError::Empty),
            Die::Sides(n) if n > self.limits.max_sides => {
                return Err(RollError::TooManySides(self.limits.max_sides));
            }
            Die::Sides(1) | Die::Fudge if d.explode => return Err(RollError::Explode(d.to_string())),
            _ if d.count == 0 => return Err(RollError::Empty),
            _ => {}
        }
        // Checked up front so an oversized count fails before any rolling.
        if d.count > self.limits.max_values {
            return Err(RollError::TooManyValues(self.limits.max_values));
        }

        let (_, max_face) = d.die.faces();
        let mut values = Vec::new();
        for _ in 0..d.count {
            let mut face = self.face(d.die)?;
            values.push(Value {
                face,
                exploded: false,
                kept: true,
            });
            while d.explode && face == max_face {
                face = self.face(d.die)?;
                values.push(Value {
                    face,
                    exploded: true,
                    kept: true,
                });
            }
        }

        if let Some(keep) = d.keep {
            // Rank by face, ties by roll order, so the result is reproducible.
            let mut order: Vec<usize> = (0..values.len()).collect();
            order.sort_by_key(|&i| (values[i].face, i));
            let n = values.len();
            let kept = usize::try_from(d.kept_count(u32::try_from(n).unwrap_or(u32::MAX))).unwrap_or(n);
            let keep_range = match keep {
                Keep::Highest(_) | Keep::DropLowest(_) => n - kept..n,
                Keep::Lowest(_) | Keep::DropHighest(_) => 0..kept,
            };
            for (rank, &i) in order.iter().enumerate() {
                values[i].kept = keep_range.contains(&rank);
            }
        }

        let mut kept = values.iter().filter(|v| v.kept);
        let total = match d.success {
            Some(c) => {
                i64::try_from(kept.filter(|v| c.op.holds(v.face, c.target)).count()).map_err(|_| RollError::Overflow)?
            }
            None => kept
                .try_fold(0i64, |acc, v| acc.checked_add(v.face))
                .ok_or(RollError::Overflow)?,
        };
        self.groups.push(Group {
            dice: d.to_string(),
            values,
        });
        Ok(total)
    }
}
