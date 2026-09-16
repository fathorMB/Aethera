use crate::parser::{Expr, Op};
use crate::CalcError;

pub fn eval(expr: &Expr) -> Result<f64, CalcError> {
    Ok(match expr {
        Expr::Num(n) => *n,
        Expr::Neg(inner) => -eval(inner)?,
        Expr::Bin(op, a, b) => {
            let (x, y) = (eval(a)?, eval(b)?);
            match op {
                Op::Add => x + y,
                Op::Sub => x - y,
                Op::Mul => x * y,
                Op::Div => {
                    if y == 0.0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    x / y
                }
                Op::Pow => x.powf(y),
            }
        }
    })
}
