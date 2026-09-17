use calc::{eval, CalcError};

fn close(input: &str, expected: f64) {
    let got = eval(input).unwrap_or_else(|e| panic!("{input}: {e:?}"));
    assert!((got - expected).abs() < 1e-9, "{input}: got {got}, expected {expected}");
}

#[test]
fn basic_precedence() {
    close("1 + 2 * 3", 7.0);
    close("(1 + 2) * 3", 9.0);
    close("10 - 4 - 3", 3.0);
    close("8 / 4 / 2", 1.0);
}

#[test]
fn power_is_right_associative() {
    close("2 ^ 3 ^ 2", 512.0);
}

#[test]
fn unary_minus_binds_looser_than_power() {
    close("-2 ^ 2", -4.0);
    close("2 ^ -1", 0.5);
}

#[test]
fn decimal_literals() {
    close("1.5 * 2", 3.0);
    close(".5 + .25", 0.75);
}

#[test]
fn errors() {
    assert_eq!(eval("1 / 0"), Err(CalcError::DivisionByZero));
    assert_eq!(eval("2 +"), Err(CalcError::UnexpectedEnd));
    assert_eq!(eval("2 $ 3"), Err(CalcError::UnexpectedChar('$')));
}
