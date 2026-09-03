//! Standard library signatures, keyed by module name.
//!
//! Deliberately separate from `checker.rs`'s walking logic, and
//! deliberately *not* shared with `aint-runtime`'s equivalent table —
//! see `docs/milestones/06-modules-stdlib/SPEC.md`'s design decisions
//! for why a small amount of duplication is accepted here rather than
//! built out into a shared registry.

use aint_ast::Type;

/// One stdlib function's fixed signature: `(param types) -> return type`.
/// `is_async` mirrors a user `async fn`: a call-expression's type is
/// `Task<return_type>` instead of `return_type` when it's set.
pub(crate) struct Signature {
    pub params: Vec<Type>,
    pub return_type: Type,
    pub is_async: bool,
}

/// The plain, monomorphic functions a module provides, as
/// `(name, signature)` pairs. `collections` isn't here: its one
/// function (`collections_length`) is genuinely polymorphic over
/// `List<T>` and is special-cased directly in `checker.rs`, the same
/// way `print` is.
pub(crate) fn module_functions(module: &str) -> Option<Vec<(&'static str, Signature)>> {
    match module {
        "math" => Some(vec![
            sig("math_sqrt", vec![Type::Float], Type::Float),
            sig("math_pow", vec![Type::Float, Type::Float], Type::Float),
            sig("math_floor", vec![Type::Float], Type::Float),
            sig("math_ceil", vec![Type::Float], Type::Float),
            sig("math_round", vec![Type::Float], Type::Float),
            sig("math_abs", vec![Type::Float], Type::Float),
            sig("math_min", vec![Type::Float, Type::Float], Type::Float),
            sig("math_max", vec![Type::Float, Type::Float], Type::Float),
        ]),
        "string" => Some(vec![
            sig("string_length", vec![Type::String], Type::Int),
            sig("string_to_upper", vec![Type::String], Type::String),
            sig("string_to_lower", vec![Type::String], Type::String),
            sig("string_trim", vec![Type::String], Type::String),
            sig(
                "string_contains",
                vec![Type::String, Type::String],
                Type::Bool,
            ),
            sig(
                "string_concat",
                vec![Type::String, Type::String],
                Type::String,
            ),
            sig(
                "string_split",
                vec![Type::String, Type::String],
                Type::List(Box::new(Type::String)),
            ),
            sig(
                "string_replace",
                vec![Type::String, Type::String, Type::String],
                Type::String,
            ),
            sig("string_url_decode", vec![Type::String], Type::String),
        ]),
        "time" => Some(vec![
            sig("time_now_seconds", vec![], Type::Int),
            async_sig("time_sleep_ms", vec![Type::Int], Type::Unit),
        ]),
        // "collections" intentionally omitted - see the doc comment above.
        "json" => Some(vec![
            sig(
                "json_get",
                vec![Type::String, Type::String],
                Type::Option(Box::new(Type::String)),
            ),
            sig(
                "json_object",
                vec![
                    Type::List(Box::new(Type::String)),
                    Type::List(Box::new(Type::String)),
                ],
                Type::String,
            ),
        ]),
        "db" => Some(vec![
            sig(
                "db_insert",
                vec![Type::String, Type::String, Type::String],
                Type::Bool,
            ),
            sig(
                "db_get",
                vec![Type::String, Type::String],
                Type::Option(Box::new(Type::String)),
            ),
            sig(
                "db_list",
                vec![Type::String],
                Type::List(Box::new(Type::String)),
            ),
            sig(
                "db_update",
                vec![Type::String, Type::String, Type::String],
                Type::Bool,
            ),
            sig("db_delete", vec![Type::String, Type::String], Type::Bool),
        ]),
        "auth" => Some(vec![
            sig("auth_hash_password", vec![Type::String], Type::String),
            sig(
                "auth_verify_password",
                vec![Type::String, Type::String],
                Type::Bool,
            ),
            sig("auth_generate_token", vec![], Type::String),
        ]),
        "log" => Some(vec![
            sig("log_info", vec![Type::String], Type::Unit),
            sig("log_error", vec![Type::String], Type::Unit),
        ]),
        "http" => Some(vec![async_sig("http_serve", vec![Type::Int], Type::Unit)]),
        _ => None,
    }
}

fn sig(name: &'static str, params: Vec<Type>, return_type: Type) -> (&'static str, Signature) {
    (
        name,
        Signature {
            params,
            return_type,
            is_async: false,
        },
    )
}

/// Like [`sig`], but for a native function that's `async` — currently
/// just `time_sleep_ms`, the one genuinely asynchronous stdlib
/// function (see `docs/milestones/07-async-concurrency/SPEC.md`).
fn async_sig(
    name: &'static str,
    params: Vec<Type>,
    return_type: Type,
) -> (&'static str, Signature) {
    (
        name,
        Signature {
            params,
            return_type,
            is_async: true,
        },
    )
}
