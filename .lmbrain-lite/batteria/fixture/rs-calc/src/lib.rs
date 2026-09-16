//! A tiny arithmetic evaluator. See README.md for the grammar.

mod eval;
mod lexer;
mod parser;

pub use parser::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum CalcError {
    /// A character the lexer does not know.
    UnexpectedChar(char),
    /// A literal that is not a valid number, such as `1.2.3`.
    BadNumber(String),
    /// A token where it cannot be, described with its debug form.
    UnexpectedToken(String),
    /// The input ended in the middle of an expression.
    UnexpectedEnd,
    DivisionByZero,
}

/// Parses without evaluating.
pub fn parse(input: &str) -> Result<Expr, CalcError> {
    let tokens = lexer::tokenize(input)?;
    parser::Parser::new(tokens).parse()
}

/// Parses and evaluates.
pub fn eval(input: &str) -> Result<f64, CalcError> {
    eval::eval(&parse(input)?)
}
