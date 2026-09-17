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

#[test]
fn hidden_mixed() {
    close("-3 * 2", -6.0);
    close("2 * -3", -6.0);
    close("--2", 2.0);
    close("(-2) ^ 2", 4.0);
    close("2 ^ 2 ^ 3 / 4", 64.0);
    close("1 - -1", 2.0);
    close("3.25*4", 13.0);
    close("2^0.5^2", 2f64.powf(0.25));
    close("-(1 + 2) ^ 2", -9.0);
    close("  7  ", 7.0);
}

#[test]
fn hidden_errors() {
    assert_eq!(eval("1.2.3"), Err(CalcError::BadNumber("1.2.3".to_string())));
    assert_eq!(eval("(1 + 2"), Err(CalcError::UnexpectedEnd));
    assert!(matches!(eval("1 2"), Err(CalcError::UnexpectedToken(_))));
    assert!(matches!(eval(")"), Err(CalcError::UnexpectedToken(_))));
    assert_eq!(eval(""), Err(CalcError::UnexpectedEnd));
    assert_eq!(eval("4 / (2 - 2)"), Err(CalcError::DivisionByZero));
}
