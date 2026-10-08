//! The four operations on `i64`. No HTTP in here.
//!
//! To add or change an operation: edit the function, add a row to the table
//! in the tests below, then register the route in `lib.rs`.

use std::fmt;

/// One of the four operations the API serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Sum,
    Sub,
    Mul,
    Div,
}

impl Operation {
    /// The path segment under `/api/`.
    pub fn name(self) -> &'static str {
        match self {
            Operation::Sum => "sum",
            Operation::Sub => "sub",
            Operation::Mul => "mul",
            Operation::Div => "div",
        }
    }

    pub fn apply(self, a: i64, b: i64) -> Result<i64, CalcError> {
        match self {
            Operation::Sum => sum(a, b),
            Operation::Sub => sub(a, b),
            Operation::Mul => mul(a, b),
            Operation::Div => div(a, b),
        }
    }
}

/// Why an operation could not produce an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalcError {
    DivisionByZero,
    Overflow,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcError::DivisionByZero => f.write_str("division by zero"),
            CalcError::Overflow => f.write_str("result does not fit in a 64-bit integer"),
        }
    }
}

impl std::error::Error for CalcError {}

pub fn sum(a: i64, b: i64) -> Result<i64, CalcError> {
    a.checked_add(b).ok_or(CalcError::Overflow)
}

pub fn sub(a: i64, b: i64) -> Result<i64, CalcError> {
    a.checked_sub(b).ok_or(CalcError::Overflow)
}

pub fn mul(a: i64, b: i64) -> Result<i64, CalcError> {
    a.checked_mul(b).ok_or(CalcError::Overflow)
}

/// Integer division, truncating toward zero: `7 / 2 = 3`, `-7 / 2 = -3`.
///
/// Dividing by zero is an error. So is `i64::MIN / -1`, the one quotient that
/// does not fit.
pub fn div(a: i64, b: i64) -> Result<i64, CalcError> {
    if b == 0 {
        return Err(CalcError::DivisionByZero);
    }
    a.checked_div(b).ok_or(CalcError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use CalcError::*;
    use Operation::*;

    #[test]
    fn results() {
        let cases: &[(Operation, i64, i64, Result<i64, CalcError>)] = &[
            // The worked example from the README.
            (Sum, 4, 1, Ok(5)),
            (Sub, 4, 1, Ok(3)),
            (Mul, 4, 1, Ok(4)),
            (Div, 7, 2, Ok(3)),
            // Truncation toward zero, both signs.
            (Div, -7, 2, Ok(-3)),
            (Div, 7, -2, Ok(-3)),
            (Div, -7, -2, Ok(3)),
            (Div, 1, 2, Ok(0)),
            (Div, -1, 2, Ok(0)),
            // Zero and negatives behave like integers do.
            (Sum, -4, 1, Ok(-3)),
            (Sub, 1, 4, Ok(-3)),
            (Mul, -4, 0, Ok(0)),
            (Div, 0, 5, Ok(0)),
            // Division by zero.
            (Div, 1, 0, Err(DivisionByZero)),
            (Div, 0, 0, Err(DivisionByZero)),
            // Overflow in every operation.
            (Sum, i64::MAX, 1, Err(Overflow)),
            (Sum, i64::MIN, -1, Err(Overflow)),
            (Sub, i64::MIN, 1, Err(Overflow)),
            (Sub, i64::MAX, -1, Err(Overflow)),
            (Mul, i64::MAX, 2, Err(Overflow)),
            (Mul, i64::MIN, -1, Err(Overflow)),
            (Div, i64::MIN, -1, Err(Overflow)),
            // The edges that do fit.
            (Sum, i64::MAX, 0, Ok(i64::MAX)),
            (Mul, i64::MIN, 1, Ok(i64::MIN)),
            (Div, i64::MIN, 1, Ok(i64::MIN)),
            (Div, i64::MAX, -1, Ok(-i64::MAX)),
        ];
        for (op, a, b, want) in cases {
            assert_eq!(op.apply(*a, *b), *want, "{} {a} {b}", op.name());
        }
    }

    #[test]
    fn names_match_the_api_paths() {
        assert_eq!(Sum.name(), "sum");
        assert_eq!(Sub.name(), "sub");
        assert_eq!(Mul.name(), "mul");
        assert_eq!(Div.name(), "div");
    }

    #[test]
    fn error_text_is_what_the_api_returns() {
        assert_eq!(DivisionByZero.to_string(), "division by zero");
        assert_eq!(
            Overflow.to_string(),
            "result does not fit in a 64-bit integer"
        );
    }
}
