//! Spec 014 (research.md §2): hand-rolled recursive-descent tokenizer +
//! parser, zero parser-library dependency. Grammar summary (this crate's
//! own concrete notation for the FR-004-FR-009a semantic requirements —
//! the spec targets the general *family* of advanced tabletop notation,
//! not one product's exact syntax, per spec.md's Assumptions):
//!
//! - dice: `NdM` (`4d6`), `dF` (Fate), `dc` (coin); `N`/`M` may each be a
//!   parenthesized sub-expression (`(2d4)d8`, `1d(1d20)`); `N` defaults
//!   to 1 when omitted (`d20`).
//! - arithmetic: `+ - * /`, parens, unary `-`.
//! - math functions: `floor(...)`, `ceil(...)`, `round(...)`, `abs(...)`.
//! - placeholders: a bare identifier not matching a reserved keyword
//!   (e.g. `STAT`).
//! - modifiers (attached directly after a dice term, any order): `kh{n}`
//!   / `kl{n}` / `dh{n}` / `dl{n}` (keep/drop, count defaults to 1),
//!   `r{cond}` (single reroll) / `rr{cond}` (recursive reroll),
//!   `x{cond}` (explode) / `xo{cond}` (explode once) — `{cond}` may be
//!   `=n`/`>n`/`>=n`/`<n`/`<=n` or a bare `n` (defaults to `=n`), or
//!   omitted entirely (defaults to "matches max face"), `min{n}` /
//!   `max{n}` (clamp), `cs{cond}` / `cf{cond}` (count successes/
//!   failures), `sf{cond}` (subtract failures by face value), `eo` /
//!   `od` (even/odd counting), `ms{n}` (margin of success).
//! - pools: `{term, term, ...}modifier` — a keep/drop modifier shared
//!   across the grouped terms' totals (FR-009).

use crate::ast::{BinOp, Condition, DiceTerm, Expr, MathFn, Modifier, Sides};
use crate::error::FormulaError;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Eq,
    Gt,
    Lt,
    Eof,
}

/// Public entry point. Tokenizes then parses `source` into a full `Expr`,
/// requiring every token be consumed (FR-011: trailing garbage like
/// `1d20 +` is a parse error, not a partial-success).
pub fn parse(source: &str) -> Result<Expr, FormulaError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_expr()?;
    match parser.peek() {
        Token::Eof => Ok(expr),
        _ => Err(FormulaError::ParseError {
            message: "unexpected trailing input".to_string(),
            position: parser.peek_pos(),
        }),
    }
}

fn tokenize(source: &str) -> Result<Vec<(Token, usize)>, FormulaError> {
    let mut tokens = Vec::new();
    let chars: Vec<(usize, char)> = source.char_indices().collect();
    let mut i = 0usize;
    let len = source.len();
    while i < chars.len() {
        let (pos, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            let mut seen_dot = false;
            while i < chars.len() {
                let ch = chars[i].1;
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !seen_dot {
                    seen_dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let end_byte = if i < chars.len() { chars[i].0 } else { len };
            let text = &source[pos..end_byte];
            let value: f64 = text.parse().map_err(|_| FormulaError::ParseError {
                message: format!("invalid number \"{text}\""),
                position: pos,
            })?;
            tokens.push((Token::Number(value), pos));
            let _ = start;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start_byte = pos;
            while i < chars.len() {
                let ch = chars[i].1;
                // Digits never continue an identifier (dice notation
                // relies on `d`/`kh`/etc. ending exactly where a numeric
                // argument begins, e.g. "4d6kh3" tokenizes as
                // Number/Ident/Number/Ident/Number, not one run).
                if ch.is_ascii_alphabetic() || ch == '_' {
                    i += 1;
                } else {
                    break;
                }
            }
            let end_byte = if i < chars.len() { chars[i].0 } else { len };
            tokens.push((Token::Ident(source[start_byte..end_byte].to_string()), pos));
        } else {
            i += 1;
            let tok = match c {
                '+' => Token::Plus,
                '-' => Token::Minus,
                '*' => Token::Star,
                '/' => Token::Slash,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                ',' => Token::Comma,
                '=' => Token::Eq,
                '>' => Token::Gt,
                '<' => Token::Lt,
                other => {
                    return Err(FormulaError::ParseError {
                        message: format!("unexpected character '{other}'"),
                        position: pos,
                    });
                }
            };
            tokens.push((tok, pos));
        }
    }
    tokens.push((Token::Eof, len));
    Ok(tokens)
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

const KEEP_HIGH: &str = "kh";
const KEEP_LOW: &str = "kl";
const DROP_HIGH: &str = "dh";
const DROP_LOW: &str = "dl";
const REROLL_ONCE: &str = "r";
const REROLL_RECURSIVE: &str = "rr";
const EXPLODE: &str = "x";
const EXPLODE_ONCE: &str = "xo";
const MIN_MOD: &str = "min";
const MAX_MOD: &str = "max";
const COUNT_SUCCESS: &str = "cs";
const COUNT_FAILURE: &str = "cf";
const SUBTRACT_FAILURE: &str = "sf";
const EVEN_MOD: &str = "eo";
const ODD_MOD: &str = "od";
const MARGIN_SUCCESS: &str = "ms";

fn modifier_keyword(ident: &str) -> Option<&'static str> {
    let lower = ident.to_ascii_lowercase();
    [
        KEEP_HIGH,
        KEEP_LOW,
        DROP_HIGH,
        DROP_LOW,
        REROLL_RECURSIVE,
        REROLL_ONCE,
        EXPLODE_ONCE,
        EXPLODE,
        MIN_MOD,
        MAX_MOD,
        COUNT_SUCCESS,
        COUNT_FAILURE,
        SUBTRACT_FAILURE,
        EVEN_MOD,
        ODD_MOD,
        MARGIN_SUCCESS,
    ]
    .into_iter()
    .find(|&kw| lower == kw)
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    fn peek_pos(&self) -> usize {
        self.tokens[self.pos].1
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].0.clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn err(&self, message: impl Into<String>) -> FormulaError {
        FormulaError::ParseError { message: message.into(), position: self.peek_pos() }
    }

    // expr := term (('+' | '-') term)*
    fn parse_expr(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_term()?;
        loop {
            match self.peek() {
                Token::Plus => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Add, Box::new(right));
                }
                Token::Minus => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Sub, Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    // term := unary (('*' | '/') unary)*
    fn parse_term(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                Token::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Mul, Box::new(right));
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinOp(Box::new(left), BinOp::Div, Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, FormulaError> {
        if matches!(self.peek(), Token::Minus) {
            self.advance();
            let inner = self.parse_unary()?;
            return Ok(Expr::Neg(Box::new(inner)));
        }
        self.parse_dice_or_primary()
    }

    /// Handles the "primary that might be the count of a dice term"
    /// case: a bare number or a parenthesized sub-expression, either of
    /// which may be immediately followed by `d`/`f`/`c` turning the
    /// whole thing into a dice term (FR-009's nested count/size case).
    fn parse_dice_or_primary(&mut self) -> Result<Expr, FormulaError> {
        // Implicit count of 1: `d20`, `dF`.
        if self.peek_starts_with_d() {
            self.consume_leading_d();
            return self.parse_dice_term(Expr::Number(1.0));
        }

        let count_candidate = match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Some(Expr::Number(n))
            }
            Token::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Some(inner)
            }
            _ => None,
        };

        if let Some(candidate) = count_candidate {
            if self.peek_starts_with_d() {
                self.consume_leading_d();
                return self.parse_dice_term(candidate);
            }
            return Ok(candidate);
        }

        self.parse_atom()
    }

    /// True when the current token is an identifier beginning with
    /// `d`/`D` — the dice operator. A bare identifier is only ever
    /// checked for this in a dice-count position (right after a number,
    /// a parenthesized sub-expression, or at the very start of a
    /// primary), so this is an intentional, documented grammar rule
    /// (not a general reservation of every word starting with 'd'): a
    /// placeholder name may not begin with 'd' where a dice term could
    /// otherwise start (e.g. `1d20 + DEX` parses `DEX` as a plain
    /// placeholder only because it's not in count position; a bare
    /// leading `DEX` as its own primary would be treated as `d`+`EX`).
    fn peek_starts_with_d(&self) -> bool {
        matches!(self.peek(), Token::Ident(name) if name.starts_with(['d', 'D']))
    }

    /// Splits the leading `d`/`D` off the current identifier token,
    /// consuming it. If the token was exactly `"d"`, this just advances
    /// past it; otherwise (e.g. `"dF"`, `"dc"`, `"dh"`) the remainder
    /// (`"F"`, `"c"`, `"h"`) is left as the new current token so sides/
    /// modifier parsing can continue reading it as an ordinary identifier.
    fn consume_leading_d(&mut self) {
        if let Token::Ident(name) = &self.tokens[self.pos].0 {
            if name.len() == 1 {
                self.advance();
            } else {
                let remainder = name[1..].to_string();
                self.tokens[self.pos].0 = Token::Ident(remainder);
            }
        }
    }

    fn parse_dice_term(&mut self, count: Expr) -> Result<Expr, FormulaError> {
        let sides = match self.peek().clone() {
            Token::Ident(name) if name.eq_ignore_ascii_case("f") => {
                self.advance();
                Sides::Fate
            }
            Token::Ident(name) if name.eq_ignore_ascii_case("c") => {
                self.advance();
                Sides::Coin
            }
            Token::Number(n) => {
                self.advance();
                Sides::Numeric(Box::new(Expr::Number(n)))
            }
            Token::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Sides::Numeric(Box::new(inner))
            }
            _ => return Err(self.err("expected die size after 'd' (a number, 'F', 'c', or a parenthesized expression)")),
        };

        let modifiers = self.parse_modifiers()?;
        Ok(Expr::Dice(DiceTerm { count: Box::new(count), sides, modifiers }))
    }

    fn parse_modifiers(&mut self) -> Result<Vec<Modifier>, FormulaError> {
        let mut modifiers = Vec::new();
        while let Token::Ident(name) = self.peek().clone() {
            let Some(kw) = modifier_keyword(&name) else { break };
            self.advance();
            let modifier = match kw {
                KEEP_HIGH => Modifier::KeepHighest(self.parse_optional_count(1)?),
                KEEP_LOW => Modifier::KeepLowest(self.parse_optional_count(1)?),
                DROP_HIGH => Modifier::DropHighest(self.parse_optional_count(1)?),
                DROP_LOW => Modifier::DropLowest(self.parse_optional_count(1)?),
                REROLL_ONCE => Modifier::Reroll(self.parse_condition_or_default()?),
                REROLL_RECURSIVE => Modifier::RerollRecursive(self.parse_condition_or_default()?),
                EXPLODE => Modifier::Explode(self.parse_condition_or_default()?),
                EXPLODE_ONCE => Modifier::ExplodeOnce(self.parse_condition_or_default()?),
                MIN_MOD => Modifier::Min(self.parse_required_int()?),
                MAX_MOD => Modifier::Max(self.parse_required_int()?),
                COUNT_SUCCESS => Modifier::CountSuccesses(self.parse_condition_or_default()?),
                COUNT_FAILURE => Modifier::CountFailures(self.parse_condition_or_default()?),
                SUBTRACT_FAILURE => Modifier::SubtractFailureValue(self.parse_condition_or_default()?),
                EVEN_MOD => Modifier::Even,
                ODD_MOD => Modifier::Odd,
                MARGIN_SUCCESS => Modifier::MarginOfSuccess(self.parse_required_int()?),
                _ => unreachable!("modifier_keyword only returns known keywords"),
            };
            modifiers.push(modifier);
        }
        Ok(modifiers)
    }

    fn parse_optional_count(&mut self, default: u32) -> Result<u32, FormulaError> {
        if let Token::Number(n) = self.peek() {
            let n = *n;
            self.advance();
            if n < 0.0 || n.fract() != 0.0 {
                return Err(self.err("expected a non-negative whole number"));
            }
            Ok(n as u32)
        } else {
            Ok(default)
        }
    }

    fn parse_required_int(&mut self) -> Result<i64, FormulaError> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(n as i64)
            }
            _ => Err(self.err("expected a number")),
        }
    }

    /// Parses an optional `(=|>|>=|<|<=)n` comparison; a bare number
    /// (no operator) defaults to `=n`; nothing at all defaults to
    /// `Condition::MaxFace`.
    fn parse_condition_or_default(&mut self) -> Result<Condition, FormulaError> {
        match self.peek().clone() {
            Token::Eq => {
                self.advance();
                Ok(Condition::Eq(self.parse_required_int()?))
            }
            Token::Gt => {
                self.advance();
                if matches!(self.peek(), Token::Eq) {
                    self.advance();
                    Ok(Condition::Gte(self.parse_required_int()?))
                } else {
                    Ok(Condition::Gt(self.parse_required_int()?))
                }
            }
            Token::Lt => {
                self.advance();
                if matches!(self.peek(), Token::Eq) {
                    self.advance();
                    Ok(Condition::Lte(self.parse_required_int()?))
                } else {
                    Ok(Condition::Lt(self.parse_required_int()?))
                }
            }
            Token::Number(n) => {
                self.advance();
                Ok(Condition::Eq(n as i64))
            }
            _ => Ok(Condition::MaxFace),
        }
    }

    fn parse_atom(&mut self) -> Result<Expr, FormulaError> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Token::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(inner)
            }
            Token::LBrace => self.parse_pool(),
            Token::Ident(name) => {
                self.advance();
                match name.to_ascii_lowercase().as_str() {
                    "floor" => self.parse_math_fn(MathFn::Floor),
                    "ceil" => self.parse_math_fn(MathFn::Ceil),
                    "round" => self.parse_math_fn(MathFn::Round),
                    "abs" => self.parse_math_fn(MathFn::Abs),
                    _ => Ok(Expr::Placeholder(name)),
                }
            }
            _ => Err(self.err("expected a number, placeholder, dice term, or '('")),
        }
    }

    fn parse_math_fn(&mut self, kind: MathFn) -> Result<Expr, FormulaError> {
        self.expect(Token::LParen)?;
        let inner = self.parse_expr()?;
        self.expect(Token::RParen)?;
        Ok(Expr::MathFn(kind, Box::new(inner)))
    }

    fn parse_pool(&mut self) -> Result<Expr, FormulaError> {
        self.expect(Token::LBrace)?;
        let mut items = vec![self.parse_expr()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            items.push(self.parse_expr()?);
        }
        self.expect(Token::RBrace)?;
        let modifiers = self.parse_modifiers()?;
        Ok(Expr::Pool(items, modifiers))
    }

    fn expect(&mut self, expected: Token) -> Result<(), FormulaError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.err(format!("expected {expected:?}")))
        }
    }
}
