use thiserror::Error;

use crate::expr::{BinOp, CmpOp, Comparison, Dice, Die, Expr, Keep};

/// The longest expression accepted, in bytes. Requests are checked before any
/// parsing work.
pub const MAX_LEN: usize = 256;
/// Nesting deeper than this is refused rather than recursed into.
const MAX_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("the expression is longer than {MAX_LEN} bytes")]
    TooLong,
    #[error("the expression nests more than {MAX_DEPTH} levels deep")]
    TooDeep,
    #[error("unexpected {found} at byte {at}")]
    Unexpected { at: usize, found: String },
    #[error("a number at byte {at} is too large")]
    NumberTooLarge { at: usize },
}

/// Parses dice notation. Whitespace is ignored; `d` and `F` are
/// case-insensitive.
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    if input.len() > MAX_LEN {
        return Err(ParseError::TooLong);
    }
    let bytes: Vec<(usize, u8)> = input
        .bytes()
        .enumerate()
        .filter(|(_, b)| !b.is_ascii_whitespace())
        .collect();
    let mut p = Parser {
        bytes,
        pos: 0,
        depth: 0,
    };
    let expr = p.expr()?;
    match p.peek() {
        None => Ok(expr),
        Some(_) => Err(p.unexpected()),
    }
}

struct Parser {
    bytes: Vec<(usize, u8)>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).map(|&(_, b)| b)
    }

    fn peek2(&self) -> Option<u8> {
        self.bytes.get(self.pos + 1).map(|&(_, b)| b)
    }

    fn at(&self) -> usize {
        self.bytes.get(self.pos).map_or(usize::MAX, |&(i, _)| i)
    }

    fn unexpected(&self) -> ParseError {
        ParseError::Unexpected {
            at: self.at(),
            found: self
                .peek()
                .map_or_else(|| "end of input".to_owned(), |b| format!("'{}'", b as char)),
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek().is_some_and(|c| c.eq_ignore_ascii_case(&b)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, ParseError>) -> Result<T, ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ParseError::TooDeep);
        }
        let out = f(self);
        self.depth -= 1;
        out
    }

    // expr := term (('+' | '-') term)*
    fn expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.term()?;
        loop {
            let op = if self.eat(b'+') {
                BinOp::Add
            } else if self.eat(b'-') {
                BinOp::Sub
            } else {
                return Ok(left);
            };
            let right = self.term()?;
            left = Expr::Bin(Box::new(left), op, Box::new(right));
        }
    }

    // term := unary ('*' unary)*
    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        while self.eat(b'*') {
            let right = self.unary()?;
            left = Expr::Bin(Box::new(left), BinOp::Mul, Box::new(right));
        }
        Ok(left)
    }

    // unary := '-' unary | atom
    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.eat(b'-') {
            return self.nested(|p| Ok(Expr::Neg(Box::new(p.unary()?))));
        }
        self.atom()
    }

    // atom := '(' expr ')' | number | number? 'd' die modifiers
    fn atom(&mut self) -> Result<Expr, ParseError> {
        if self.eat(b'(') {
            let inner = self.nested(Self::expr)?;
            if !self.eat(b')') {
                return Err(self.unexpected());
            }
            return Ok(inner);
        }
        let count = self.number()?;
        if self.eat(b'd') {
            return self.dice(count.unwrap_or(1)).map(Expr::Dice);
        }
        count.map(Expr::Const).ok_or_else(|| self.unexpected())
    }

    fn dice(&mut self, count: u32) -> Result<Dice, ParseError> {
        let die = if self.eat(b'%') {
            Die::Percent
        } else if self.eat(b'f') {
            Die::Fudge
        } else {
            Die::Sides(self.number()?.ok_or_else(|| self.unexpected())?)
        };
        let explode = self.eat(b'!');
        let keep = self.keep()?;
        let success = self.comparison()?;
        Ok(Dice {
            count,
            die,
            explode,
            keep,
            success,
        })
    }

    fn keep(&mut self) -> Result<Option<Keep>, ParseError> {
        let kind = match (
            self.peek().map(|b| b.to_ascii_lowercase()),
            self.peek2().map(|b| b.to_ascii_lowercase()),
        ) {
            (Some(b'k'), Some(b'h')) => Keep::Highest as fn(u32) -> Keep,
            (Some(b'k'), Some(b'l')) => Keep::Lowest,
            (Some(b'd'), Some(b'h')) => Keep::DropHighest,
            (Some(b'd'), Some(b'l')) => Keep::DropLowest,
            _ => return Ok(None),
        };
        self.pos += 2;
        let n = self.number()?.ok_or_else(|| self.unexpected())?;
        Ok(Some(kind(n)))
    }

    fn comparison(&mut self) -> Result<Option<Comparison>, ParseError> {
        let op = if self.eat(b'>') {
            if self.eat(b'=') { CmpOp::Ge } else { CmpOp::Gt }
        } else if self.eat(b'<') {
            if self.eat(b'=') { CmpOp::Le } else { CmpOp::Lt }
        } else if self.eat(b'=') {
            CmpOp::Eq
        } else {
            return Ok(None);
        };
        let negative = self.eat(b'-');
        let n = i64::from(self.number()?.ok_or_else(|| self.unexpected())?);
        Ok(Some(Comparison {
            op,
            target: if negative { -n } else { n },
        }))
    }

    fn number(&mut self) -> Result<Option<u32>, ParseError> {
        let start = self.at();
        let mut value: Option<u32> = None;
        while let Some(b) = self.peek().filter(u8::is_ascii_digit) {
            let digit = u32::from(b - b'0');
            value = Some(
                value
                    .unwrap_or(0)
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(digit))
                    .ok_or(ParseError::NumberTooLarge { at: start })?,
            );
            self.pos += 1;
        }
        Ok(value)
    }
}
