//! Pratt parser. Each infix operator has a pair of binding powers (left, right): a loop keeps
//! absorbing operators whose left power is at least the current minimum, and parses the right
//! operand with the operator's right power. Left < right makes an operator left-associative,
//! left > right makes it right-associative.

use crate::lexer::Token;
use crate::CalcError;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

fn infix_binding_power(token: Token) -> Option<(Op, u8, u8)> {
    match token {
        Token::Plus => Some((Op::Add, 1, 2)),
        Token::Minus => Some((Op::Sub, 1, 2)),
        Token::Star => Some((Op::Mul, 3, 4)),
        Token::Slash => Some((Op::Div, 3, 4)),
        Token::Caret => Some((Op::Pow, 5, 6)),
        _ => None,
    }
}

/// Right binding power of unary minus.
const PREFIX_MINUS: u8 = 7;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.peek();
        self.pos += 1;
        t
    }

    pub fn parse(mut self) -> Result<Expr, CalcError> {
        let expr = self.expr(0)?;
        match self.peek() {
            None => Ok(expr),
            Some(t) => Err(CalcError::UnexpectedToken(format!("{t:?}"))),
        }
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, CalcError> {
        let mut lhs = match self.next() {
            Some(Token::Num(n)) => Expr::Num(n),
            Some(Token::Minus) => Expr::Neg(Box::new(self.expr(PREFIX_MINUS)?)),
            Some(Token::LParen) => {
                let inner = self.expr(0)?;
                match self.next() {
                    Some(Token::RParen) => inner,
                    Some(t) => return Err(CalcError::UnexpectedToken(format!("{t:?}"))),
                    None => return Err(CalcError::UnexpectedEnd),
                }
            }
            Some(t) => return Err(CalcError::UnexpectedToken(format!("{t:?}"))),
            None => return Err(CalcError::UnexpectedEnd),
        };
        loop {
            let Some(token) = self.peek() else { break };
            if token == Token::RParen {
                break;
            }
            let Some((op, l_bp, r_bp)) = infix_binding_power(token) else {
                return Err(CalcError::UnexpectedToken(format!("{token:?}")));
            };
            if l_bp < min_bp {
                break;
            }
            self.next();
            let rhs = self.expr(r_bp)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
}
