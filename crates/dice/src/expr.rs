use std::fmt;

use serde::{Deserialize, Serialize};

/// A parsed dice expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    /// A non-negative integer; negative numbers are [`Expr::Neg`].
    Const(u32),
    Dice(Dice),
    Neg(Box<Expr>),
    Bin(Box<Expr>, BinOp, Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
}

/// `count` dice of one kind, with optional explosion, keep/drop and success
/// counting, applied in that order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dice {
    pub count: u32,
    pub die: Die,
    pub explode: bool,
    pub keep: Option<Keep>,
    pub success: Option<Comparison>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Die {
    Sides(u32),
    /// `d%`: 1 to 100.
    Percent,
    /// `dF`: -1, 0 or +1.
    Fudge,
}

impl Die {
    pub fn faces(self) -> (i64, i64) {
        match self {
            Self::Sides(n) => (1, i64::from(n)),
            Self::Percent => (1, 100),
            Self::Fudge => (-1, 1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keep {
    Highest(u32),
    Lowest(u32),
    DropHighest(u32),
    DropLowest(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comparison {
    pub op: CmpOp,
    pub target: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
}

impl CmpOp {
    pub fn holds(self, value: i64, target: i64) -> bool {
        match self {
            Self::Ge => value >= target,
            Self::Le => value <= target,
            Self::Gt => value > target,
            Self::Lt => value < target,
            Self::Eq => value == target,
        }
    }
}

/// The range a total can fall in. `None` is unbounded on that side (an
/// exploding die has no maximum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min: Option<i128>,
    pub max: Option<i128>,
}

impl Bounds {
    fn exact(v: i128) -> Self {
        Self {
            min: Some(v),
            max: Some(v),
        }
    }

    pub fn contains(&self, v: i64) -> bool {
        let v = i128::from(v);
        self.min.is_none_or(|m| m <= v) && self.max.is_none_or(|m| v <= m)
    }
}

impl Dice {
    /// How many dice the keep/drop step keeps, given `rolled` values.
    pub(crate) fn kept_count(&self, rolled: u32) -> u32 {
        match self.keep {
            None => rolled,
            Some(Keep::Highest(n) | Keep::Lowest(n)) => n.min(rolled),
            Some(Keep::DropHighest(n) | Keep::DropLowest(n)) => rolled.saturating_sub(n),
        }
    }

    fn bounds(&self) -> Bounds {
        let kept_min = i128::from(self.kept_count(self.count));
        // Explosions only add dice, so keep-highest/lowest keeps the same
        // number and drop-n keeps at least as many.
        let kept_max = if self.explode {
            None
        } else {
            Some(i128::from(self.kept_count(self.count)))
        };
        if let Some(c) = self.success {
            let (lo, hi) = self.die.faces();
            let can_fail = (lo..=hi).any(|f| !c.op.holds(f, c.target));
            let can_succeed = (lo..=hi).any(|f| c.op.holds(f, c.target));
            return Bounds {
                min: Some(if can_fail { 0 } else { kept_min }),
                max: if can_succeed { kept_max } else { Some(0) },
            };
        }
        let (lo, hi) = self.die.faces();
        let (lo, hi) = (i128::from(lo), i128::from(hi));
        match kept_max {
            // Only dice with positive faces explode (see `RollError::Explode`),
            // so more dice can only raise the total.
            None => Bounds {
                min: Some(kept_min * lo),
                max: None,
            },
            Some(k) => Bounds {
                min: Some((k * lo).min(k * hi)),
                max: Some((k * lo).max(k * hi)),
            },
        }
    }
}

impl Expr {
    /// The range of totals this expression can produce, without rolling.
    #[allow(clippy::many_single_char_names)] // interval arithmetic reads best as a..d
    pub fn bounds(&self) -> Bounds {
        match self {
            Self::Const(n) => Bounds::exact(i128::from(*n)),
            Self::Dice(d) => d.bounds(),
            Self::Neg(e) => {
                let b = e.bounds();
                Bounds {
                    min: b.max.map(|v| -v),
                    max: b.min.map(|v| -v),
                }
            }
            Self::Bin(l, op, r) => {
                let (l, r) = (l.bounds(), r.bounds());
                match op {
                    BinOp::Add => Bounds {
                        min: l.min.zip(r.min).map(|(a, b)| a + b),
                        max: l.max.zip(r.max).map(|(a, b)| a + b),
                    },
                    BinOp::Sub => Bounds {
                        min: l.min.zip(r.max).map(|(a, b)| a - b),
                        max: l.max.zip(r.min).map(|(a, b)| a - b),
                    },
                    BinOp::Mul => match (l.min, l.max, r.min, r.max) {
                        (Some(a), Some(b), Some(c), Some(d)) => {
                            let p = [a * c, a * d, b * c, b * d];
                            Bounds {
                                min: p.iter().min().copied(),
                                max: p.iter().max().copied(),
                            }
                        }
                        _ => Bounds { min: None, max: None },
                    },
                }
            }
        }
    }

    fn precedence(&self) -> u8 {
        match self {
            Self::Bin(_, BinOp::Add | BinOp::Sub, _) => 1,
            Self::Bin(_, BinOp::Mul, _) => 2,
            Self::Neg(_) => 3,
            Self::Const(_) | Self::Dice(_) => 4,
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let child = |f: &mut fmt::Formatter<'_>, e: &Expr, min: u8| {
            if e.precedence() < min {
                write!(f, "({e})")
            } else {
                write!(f, "{e}")
            }
        };
        match self {
            Self::Const(n) => write!(f, "{n}"),
            Self::Dice(d) => write!(f, "{d}"),
            Self::Neg(e) => {
                f.write_str("-")?;
                child(f, e, 3)
            }
            Self::Bin(l, op, r) => {
                let p = self.precedence();
                child(f, l, p)?;
                f.write_str(match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                })?;
                // Left-associative: an equal-precedence right child needs parens.
                child(f, r, p + 1)
            }
        }
    }
}

impl fmt::Display for Dice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d", self.count)?;
        match self.die {
            Die::Sides(n) => write!(f, "{n}")?,
            Die::Percent => f.write_str("%")?,
            Die::Fudge => f.write_str("F")?,
        }
        if self.explode {
            f.write_str("!")?;
        }
        match self.keep {
            None => {}
            Some(Keep::Highest(n)) => write!(f, "kh{n}")?,
            Some(Keep::Lowest(n)) => write!(f, "kl{n}")?,
            Some(Keep::DropHighest(n)) => write!(f, "dh{n}")?,
            Some(Keep::DropLowest(n)) => write!(f, "dl{n}")?,
        }
        if let Some(c) = self.success {
            let op = match c.op {
                CmpOp::Ge => ">=",
                CmpOp::Le => "<=",
                CmpOp::Gt => ">",
                CmpOp::Lt => "<",
                CmpOp::Eq => "=",
            };
            write!(f, "{op}{}", c.target)?;
        }
        Ok(())
    }
}
