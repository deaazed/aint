//! Native implementations of the stdlib functions gated behind
//! `import`, plus the per-module binding tables `Interpreter` uses when
//! executing `StmtKind::Import`.
//!
//! Deliberately separate from `interpreter.rs`, and deliberately *not*
//! shared with `aint-typechecker`'s equivalent signature tables — see
//! `docs/milestones/06-modules-stdlib/SPEC.md`'s design decisions for
//! why a small amount of duplication is accepted here.
//!
//! `print` isn't handled here: it needs the interpreter's output
//! writer, which this module has no access to, so `Interpreter::call`
//! keeps handling it directly.

use std::time::{SystemTime, UNIX_EPOCH};

use aint_ast::Span;

use crate::error::RuntimeError;
use crate::value::{NativeFunction, Value};

/// The native functions `import <module>` should bind, and under what
/// names. `None` for an unrecognized module name.
pub(crate) fn module_bindings(module: &str) -> Option<Vec<(&'static str, NativeFunction)>> {
    match module {
        "math" => Some(vec![
            ("math_sqrt", NativeFunction::MathSqrt),
            ("math_pow", NativeFunction::MathPow),
            ("math_floor", NativeFunction::MathFloor),
            ("math_ceil", NativeFunction::MathCeil),
            ("math_round", NativeFunction::MathRound),
            ("math_abs", NativeFunction::MathAbs),
            ("math_min", NativeFunction::MathMin),
            ("math_max", NativeFunction::MathMax),
        ]),
        "string" => Some(vec![
            ("string_length", NativeFunction::StringLength),
            ("string_to_upper", NativeFunction::StringToUpper),
            ("string_to_lower", NativeFunction::StringToLower),
            ("string_trim", NativeFunction::StringTrim),
            ("string_contains", NativeFunction::StringContains),
            ("string_concat", NativeFunction::StringConcat),
        ]),
        "time" => Some(vec![
            ("time_now_seconds", NativeFunction::TimeNowSeconds),
            ("time_sleep_ms", NativeFunction::TimeSleepMs),
        ]),
        "collections" => Some(vec![(
            "collections_length",
            NativeFunction::CollectionsLength,
        )]),
        _ => None,
    }
}

/// Calls any native function except [`NativeFunction::Print`].
pub(crate) fn call(
    native: NativeFunction,
    args: Vec<Value>,
    span: Span,
) -> Result<Value, RuntimeError> {
    match native {
        NativeFunction::Print => unreachable!("Print is handled directly in Interpreter::call"),
        NativeFunction::TimeSleepMs => {
            unreachable!("async natives are handled by Interpreter::eval_await, not stdlib::call")
        }
        NativeFunction::MathSqrt => {
            let [x] = one(native, args, span)?;
            Ok(Value::Float(float(x, span)?.sqrt()))
        }
        NativeFunction::MathPow => {
            let [base, exponent] = two(native, args, span)?;
            Ok(Value::Float(
                float(base, span)?.powf(float(exponent, span)?),
            ))
        }
        NativeFunction::MathFloor => {
            let [x] = one(native, args, span)?;
            Ok(Value::Float(float(x, span)?.floor()))
        }
        NativeFunction::MathCeil => {
            let [x] = one(native, args, span)?;
            Ok(Value::Float(float(x, span)?.ceil()))
        }
        NativeFunction::MathRound => {
            let [x] = one(native, args, span)?;
            Ok(Value::Float(float(x, span)?.round()))
        }
        NativeFunction::MathAbs => {
            let [x] = one(native, args, span)?;
            Ok(Value::Float(float(x, span)?.abs()))
        }
        NativeFunction::MathMin => {
            let [a, b] = two(native, args, span)?;
            Ok(Value::Float(float(a, span)?.min(float(b, span)?)))
        }
        NativeFunction::MathMax => {
            let [a, b] = two(native, args, span)?;
            Ok(Value::Float(float(a, span)?.max(float(b, span)?)))
        }
        NativeFunction::StringLength => {
            let [s] = one(native, args, span)?;
            Ok(Value::Int(string(&s, span)?.chars().count() as i64))
        }
        NativeFunction::StringToUpper => {
            let [s] = one(native, args, span)?;
            Ok(Value::String(string(&s, span)?.to_uppercase()))
        }
        NativeFunction::StringToLower => {
            let [s] = one(native, args, span)?;
            Ok(Value::String(string(&s, span)?.to_lowercase()))
        }
        NativeFunction::StringTrim => {
            let [s] = one(native, args, span)?;
            Ok(Value::String(string(&s, span)?.trim().to_string()))
        }
        NativeFunction::StringContains => {
            let [s, needle] = two(native, args, span)?;
            Ok(Value::Bool(
                string(&s, span)?.contains(string(&needle, span)?),
            ))
        }
        NativeFunction::StringConcat => {
            let [a, b] = two(native, args, span)?;
            Ok(Value::String(format!(
                "{}{}",
                string(&a, span)?,
                string(&b, span)?
            )))
        }
        NativeFunction::TimeNowSeconds => {
            let [] = zero(native, args, span)?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            Ok(Value::Int(now.as_secs() as i64))
        }
        NativeFunction::CollectionsLength => {
            let [list] = one(native, args, span)?;
            match list {
                Value::List(items) => Ok(Value::Int(items.len() as i64)),
                other => Err(RuntimeError::TypeMismatch {
                    message: format!(
                        "collections_length expects a List, found {}",
                        other.type_name()
                    ),
                    span,
                }),
            }
        }
    }
}

fn zero(native: NativeFunction, args: Vec<Value>, span: Span) -> Result<[Value; 0], RuntimeError> {
    let found = args.len();
    args.try_into().map_err(|_| RuntimeError::ArityMismatch {
        name: native.name().to_string(),
        expected: 0,
        found,
        span,
    })
}

pub(crate) fn one(
    native: NativeFunction,
    args: Vec<Value>,
    span: Span,
) -> Result<[Value; 1], RuntimeError> {
    let found = args.len();
    args.try_into().map_err(|_| RuntimeError::ArityMismatch {
        name: native.name().to_string(),
        expected: 1,
        found,
        span,
    })
}

fn two(native: NativeFunction, args: Vec<Value>, span: Span) -> Result<[Value; 2], RuntimeError> {
    let found = args.len();
    args.try_into().map_err(|_| RuntimeError::ArityMismatch {
        name: native.name().to_string(),
        expected: 2,
        found,
        span,
    })
}

pub(crate) fn int(value: Value, span: Span) -> Result<i64, RuntimeError> {
    match value {
        Value::Int(n) => Ok(n),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected Int, found {}", other.type_name()),
            span,
        }),
    }
}

fn float(value: Value, span: Span) -> Result<f64, RuntimeError> {
    match value {
        Value::Float(f) => Ok(f),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected Float, found {}", other.type_name()),
            span,
        }),
    }
}

fn string(value: &Value, span: Span) -> Result<&str, RuntimeError> {
    match value {
        Value::String(s) => Ok(s),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected String, found {}", other.type_name()),
            span,
        }),
    }
}
