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
///
/// `pub`, not `pub(crate)`, since milestone 22's bytecode VM
/// (`aint-vm`) needs the exact same name-to-native table `Interpreter`
/// uses for `StmtKind::Import` — resolving every native call at
/// compile time instead of re-deriving this table is the whole point,
/// so it has to be the same table, not a second one drifting from it.
pub fn module_bindings(module: &str) -> Option<Vec<(&'static str, NativeFunction)>> {
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
        "distribution" => Some(vec![
            (
                "distribution_probability",
                NativeFunction::DistributionProbability,
            ),
            ("distribution_argmax", NativeFunction::DistributionArgmax),
            ("distribution_entropy", NativeFunction::DistributionEntropy),
            ("distribution_sample", NativeFunction::DistributionSample),
            (
                "distribution_require_confidence",
                NativeFunction::DistributionRequireConfidence,
            ),
        ]),
        "option" => Some(vec![
            ("option_is_some", NativeFunction::OptionIsSome),
            ("option_unwrap", NativeFunction::OptionUnwrap),
        ]),
        _ => None,
    }
}

/// Calls any native function except [`NativeFunction::Print`]. `pub`
/// for the same reason as `module_bindings` — `aint-vm` reuses this
/// exact implementation rather than re-deriving stdlib semantics.
pub fn call(native: NativeFunction, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
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
        NativeFunction::DistributionProbability => {
            let [dist, value] = two(native, args, span)?;
            let (_, entries) = distribution(dist, span)?;
            let variant = enum_variant(&value, span)?;
            let probability = entries
                .iter()
                .find(|(v, _)| v == variant)
                .map_or(0.0, |(_, p)| *p);
            Ok(Value::Float(probability))
        }
        NativeFunction::DistributionArgmax => {
            let [dist] = one(native, args, span)?;
            let (name, entries) = distribution(dist, span)?;
            let (variant, _) = argmax_entry(&entries, span)?;
            Ok(Value::Enum(name, variant.clone()))
        }
        NativeFunction::DistributionEntropy => {
            let [dist] = one(native, args, span)?;
            let (_, entries) = distribution(dist, span)?;
            let entropy = -entries
                .iter()
                .map(|(_, p)| if *p > 0.0 { p * p.log2() } else { 0.0 })
                .sum::<f64>();
            Ok(Value::Float(entropy))
        }
        NativeFunction::DistributionSample => {
            let [dist] = one(native, args, span)?;
            let (name, entries) = distribution(dist, span)?;
            let roll: f64 = rand::random();
            let mut cumulative = 0.0;
            let mut chosen = entries.last().map(|(v, _)| v.clone());
            for (variant, probability) in &entries {
                cumulative += probability;
                if roll < cumulative {
                    chosen = Some(variant.clone());
                    break;
                }
            }
            let variant = chosen.ok_or_else(|| RuntimeError::TypeMismatch {
                message: "distribution has no entries to sample from".to_string(),
                span,
            })?;
            Ok(Value::Enum(name, variant))
        }
        NativeFunction::DistributionRequireConfidence => {
            let [dist, threshold] = two(native, args, span)?;
            let (name, entries) = distribution(dist, span)?;
            let threshold = float(threshold, span)?;
            let (variant, probability) = argmax_entry(&entries, span)?;
            if *probability >= threshold {
                Ok(Value::Option(Some(Box::new(Value::Enum(
                    name,
                    variant.clone(),
                )))))
            } else {
                Ok(Value::Option(None))
            }
        }
        NativeFunction::OptionIsSome => {
            let [opt] = one(native, args, span)?;
            match opt {
                Value::Option(inner) => Ok(Value::Bool(inner.is_some())),
                other => Err(RuntimeError::TypeMismatch {
                    message: format!(
                        "option_is_some expects an Option, found {}",
                        other.type_name()
                    ),
                    span,
                }),
            }
        }
        NativeFunction::OptionUnwrap => {
            let [opt] = one(native, args, span)?;
            match opt {
                Value::Option(Some(inner)) => Ok(*inner),
                Value::Option(None) => Err(RuntimeError::TypeMismatch {
                    message: "option_unwrap called on None".to_string(),
                    span,
                }),
                other => Err(RuntimeError::TypeMismatch {
                    message: format!(
                        "option_unwrap expects an Option, found {}",
                        other.type_name()
                    ),
                    span,
                }),
            }
        }
    }
}

fn distribution(value: Value, span: Span) -> Result<(String, Vec<(String, f64)>), RuntimeError> {
    match value {
        Value::Distribution(name, entries) => Ok((name, entries)),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected a Distribution, found {}", other.type_name()),
            span,
        }),
    }
}

fn enum_variant(value: &Value, span: Span) -> Result<&str, RuntimeError> {
    match value {
        Value::Enum(_, variant) => Ok(variant),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected an enum value, found {}", other.type_name()),
            span,
        }),
    }
}

/// The first entry with the strictly-highest probability — ties keep
/// whichever entry came first, a deterministic, documented choice
/// rather than an accident of iteration order.
fn argmax_entry(entries: &[(String, f64)], span: Span) -> Result<&(String, f64), RuntimeError> {
    entries
        .iter()
        .fold(None, |best, entry| match best {
            None => Some(entry),
            Some(current_best) if entry.1 > current_best.1 => Some(entry),
            Some(current_best) => Some(current_best),
        })
        .ok_or_else(|| RuntimeError::TypeMismatch {
            message: "distribution has no entries".to_string(),
            span,
        })
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
