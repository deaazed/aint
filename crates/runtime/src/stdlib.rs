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

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use aint_ast::Span;
use rand::RngCore;

use crate::db;
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
            ("string_split", NativeFunction::StringSplit),
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
        "json" => Some(vec![
            ("json_get", NativeFunction::JsonGet),
            ("json_object", NativeFunction::JsonObject),
        ]),
        "db" => Some(vec![
            ("db_insert", NativeFunction::DbInsert),
            ("db_get", NativeFunction::DbGet),
            ("db_list", NativeFunction::DbList),
            ("db_update", NativeFunction::DbUpdate),
            ("db_delete", NativeFunction::DbDelete),
        ]),
        "auth" => Some(vec![
            ("auth_hash_password", NativeFunction::AuthHashPassword),
            ("auth_verify_password", NativeFunction::AuthVerifyPassword),
            ("auth_generate_token", NativeFunction::AuthGenerateToken),
        ]),
        "log" => Some(vec![
            ("log_info", NativeFunction::LogInfo),
            ("log_error", NativeFunction::LogError),
        ]),
        "http" => Some(vec![("http_serve", NativeFunction::HttpServe)]),
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
        NativeFunction::StringSplit => {
            let [s, sep] = two(native, args, span)?;
            let s = string(&s, span)?;
            let sep = string(&sep, span)?;
            let parts: Vec<Value> = if sep.is_empty() {
                vec![Value::String(s.to_string())]
            } else {
                s.split(sep)
                    .map(|part| Value::String(part.to_string()))
                    .collect()
            };
            Ok(Value::List(parts))
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
        NativeFunction::HttpServe => {
            unreachable!("http_serve is handled directly in Interpreter::run_async_native")
        }
        NativeFunction::JsonGet => {
            let [json, key] = two(native, args, span)?;
            let json = string(&json, span)?;
            let key = string(&key, span)?;
            let parsed: serde_json::Value =
                serde_json::from_str(json).map_err(|err| RuntimeError::TypeMismatch {
                    message: format!("json_get: invalid JSON: {err}"),
                    span,
                })?;
            match parsed.get(key).and_then(|v| v.as_str()) {
                Some(value) => Ok(Value::Option(Some(Box::new(Value::String(
                    value.to_string(),
                ))))),
                None => Ok(Value::Option(None)),
            }
        }
        NativeFunction::JsonObject => {
            let [keys, values] = two(native, args, span)?;
            let keys = list_of_strings(keys, span)?;
            let values = list_of_strings(values, span)?;
            if keys.len() != values.len() {
                return Err(RuntimeError::TypeMismatch {
                    message: format!(
                        "json_object: {} keys but {} values",
                        keys.len(),
                        values.len()
                    ),
                    span,
                });
            }
            let mut object = serde_json::Map::new();
            for (key, value) in keys.into_iter().zip(values) {
                object.insert(key, serde_json::Value::String(value));
            }
            Ok(Value::String(
                serde_json::to_string(&object).expect("a flat string map always serializes"),
            ))
        }
        NativeFunction::DbInsert => {
            let [table, id, record] = three(native, args, span)?;
            let table = string(&table, span)?;
            let id = string(&id, span)?;
            let record = string(&record, span)?;
            let ok = db::insert(Path::new(db::DEFAULT_DB_DIR), table, id, record, span)?;
            Ok(Value::Bool(ok))
        }
        NativeFunction::DbGet => {
            let [table, id] = two(native, args, span)?;
            let table = string(&table, span)?;
            let id = string(&id, span)?;
            let found = db::get(Path::new(db::DEFAULT_DB_DIR), table, id, span)?;
            Ok(Value::Option(found.map(|s| Box::new(Value::String(s)))))
        }
        NativeFunction::DbList => {
            let [table] = one(native, args, span)?;
            let table = string(&table, span)?;
            let records = db::list(Path::new(db::DEFAULT_DB_DIR), table, span)?;
            Ok(Value::List(
                records.into_iter().map(Value::String).collect(),
            ))
        }
        NativeFunction::DbUpdate => {
            let [table, id, record] = three(native, args, span)?;
            let table = string(&table, span)?;
            let id = string(&id, span)?;
            let record = string(&record, span)?;
            let ok = db::update(Path::new(db::DEFAULT_DB_DIR), table, id, record, span)?;
            Ok(Value::Bool(ok))
        }
        NativeFunction::DbDelete => {
            let [table, id] = two(native, args, span)?;
            let table = string(&table, span)?;
            let id = string(&id, span)?;
            let ok = db::delete(Path::new(db::DEFAULT_DB_DIR), table, id, span)?;
            Ok(Value::Bool(ok))
        }
        NativeFunction::AuthHashPassword => {
            let [password] = one(native, args, span)?;
            let password = string(&password, span)?;
            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|err| {
                RuntimeError::TypeMismatch {
                    message: format!("auth_hash_password: {err}"),
                    span,
                }
            })?;
            Ok(Value::String(hash))
        }
        NativeFunction::AuthVerifyPassword => {
            let [password, hash] = two(native, args, span)?;
            let password = string(&password, span)?;
            let hash = string(&hash, span)?;
            let matches = bcrypt::verify(password, hash).unwrap_or(false);
            Ok(Value::Bool(matches))
        }
        NativeFunction::AuthGenerateToken => {
            let [] = zero(native, args, span)?;
            let mut bytes = [0u8; 24];
            rand::thread_rng().fill_bytes(&mut bytes);
            let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            Ok(Value::String(token))
        }
        NativeFunction::LogInfo => {
            let [message] = one(native, args, span)?;
            log_line("INFO", string(&message, span)?);
            Ok(Value::Unit)
        }
        NativeFunction::LogError => {
            let [message] = one(native, args, span)?;
            log_line("ERROR", string(&message, span)?);
            Ok(Value::Unit)
        }
    }
}

fn log_line(level: &str, message: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    eprintln!("[{now} {level}] {message}");
}

fn list_of_strings(value: Value, span: Span) -> Result<Vec<String>, RuntimeError> {
    match value {
        Value::List(items) => items
            .into_iter()
            .map(|item| match item {
                Value::String(s) => Ok(s),
                other => Err(RuntimeError::TypeMismatch {
                    message: format!(
                        "expected a List<String>, found a List containing {}",
                        other.type_name()
                    ),
                    span,
                }),
            })
            .collect(),
        other => Err(RuntimeError::TypeMismatch {
            message: format!("expected List<String>, found {}", other.type_name()),
            span,
        }),
    }
}

fn three(native: NativeFunction, args: Vec<Value>, span: Span) -> Result<[Value; 3], RuntimeError> {
    let found = args.len();
    args.try_into().map_err(|_| RuntimeError::ArityMismatch {
        name: native.name().to_string(),
        expected: 3,
        found,
        span,
    })
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
