//! Resolves cross-file `import "path" as alias` (milestone 29) into one
//! flat [`Program`] before any other crate in the pipeline ever sees it —
//! the type checker, interpreter, IR lowering, and bytecode VM compiler
//! are all completely unchanged by this: they still only ever operate on
//! a single, ordinary, flat `Program`, exactly as they did before this
//! crate existed. See `docs/milestones/29-modularity/SPEC.md`.
//!
//! `aint check`/`run`/`test`/`run --vm` all call [`load`] where they used
//! to call [`aint_parser::parse_source`] directly. `aint fmt` does not —
//! formatting a file must reproduce its own literal `import "..." as
//! ...` statements, not resolve them.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use aint_ast::{Block, Expr, ExprKind, Param, Program, Span, Stmt, StmtKind, Type};

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Io {
        path: String,
        message: String,
    },
    Parse {
        path: String,
        message: String,
    },
    /// A depends on B depends on ... depends on A, reported with the
    /// full chain — the same shape `aint-package`'s `resolve.rs` uses
    /// for manifest dependency cycles.
    Cycle {
        cycle: Vec<String>,
    },
    /// The same file, reached from two different places in the import
    /// graph. Not supported in v1 — see SPEC.md's "No diamond imports."
    DuplicateImport {
        path: String,
    },
    /// Two `import "..." as x` statements in the same file both using
    /// alias `x`.
    DuplicateAlias {
        file: String,
        alias: String,
    },
    /// A statement other than `fn`/`enum`/`tool`/`infer`/`import` at
    /// the top level of an *imported* file (the entry file has no such
    /// restriction).
    IllegalTopLevelStatement {
        file: String,
        span: Span,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, message } => write!(f, "could not read {path}: {message}"),
            LoadError::Parse { path, message } => write!(f, "{path}: {message}"),
            LoadError::Cycle { cycle } => {
                write!(f, "cyclic import: {}", cycle.join(" -> "))
            }
            LoadError::DuplicateImport { path } => write!(
                f,
                "{path} is imported from more than one place — diamond imports aren't supported yet, see docs/milestones/29-modularity/SPEC.md"
            ),
            LoadError::DuplicateAlias { file, alias } => {
                write!(f, "{file}: two imports both use the alias `{alias}`")
            }
            LoadError::IllegalTopLevelStatement { file, span } => write!(
                f,
                "{file}:{}: only fn/enum/tool/infer/import declarations are allowed at the top level of an imported file",
                span.start
            ),
        }
    }
}

impl std::error::Error for LoadError {}

/// Loads `entry_path`, resolving every cross-file `import "path" as
/// alias` it (transitively) reaches, into one flat [`Program`].
pub fn load(entry_path: &Path) -> Result<Program, LoadError> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let statements = resolve_file(entry_path, None, &mut visited, &mut stack)?;
    Ok(Program { statements })
}

/// Resolves one file into its final, flattened statement list.
/// `alias` is `None` only for the entry file — every imported file has
/// one, and gets every one of its top-level declarations (and every
/// reference to them, inside its own source) prefixed with it.
fn resolve_file(
    path: &Path,
    alias: Option<&str>,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Vec<Stmt>, LoadError> {
    let canonical = path.canonicalize().map_err(|err| LoadError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;

    if stack.contains(&canonical) {
        let start = stack
            .iter()
            .position(|p| p == &canonical)
            .expect("just checked");
        let mut cycle: Vec<String> = stack[start..]
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        cycle.push(canonical.display().to_string());
        return Err(LoadError::Cycle { cycle });
    }
    if visited.contains(&canonical) {
        return Err(LoadError::DuplicateImport {
            path: canonical.display().to_string(),
        });
    }
    visited.insert(canonical.clone());
    stack.push(canonical.clone());

    let source = std::fs::read_to_string(&canonical).map_err(|err| LoadError::Io {
        path: canonical.display().to_string(),
        message: err.to_string(),
    })?;
    let program = aint_parser::parse_source(&source).map_err(|err| LoadError::Parse {
        path: canonical.display().to_string(),
        message: err.to_string(),
    })?;

    let dir = canonical
        .parent()
        .expect("a canonicalized file path always has a parent")
        .to_path_buf();

    let mut flat = Vec::new();
    let mut seen_aliases = HashSet::new();
    for stmt in program.statements {
        match stmt.kind {
            StmtKind::ImportFile {
                path: rel_path,
                alias: import_alias,
            } => {
                if !seen_aliases.insert(import_alias.clone()) {
                    return Err(LoadError::DuplicateAlias {
                        file: canonical.display().to_string(),
                        alias: import_alias,
                    });
                }
                let imported =
                    resolve_file(&dir.join(&rel_path), Some(&import_alias), visited, stack)?;
                flat.extend(imported);
            }
            other => {
                if alias.is_some() {
                    validate_module_top_level(&other, stmt.span, &canonical)?;
                }
                flat.push(Stmt::new(other, stmt.span));
            }
        }
    }

    stack.pop();

    match alias {
        Some(alias) => {
            let map = collect_rename_map(&flat, alias);
            Ok(flat.into_iter().map(|s| rename_stmt(s, &map)).collect())
        }
        None => Ok(flat),
    }
}

/// Only `fn`/`enum`/`tool`/`infer`/`import` may appear at the top level
/// of a file reached *through* an `import "..." as ...` — see SPEC.md.
fn validate_module_top_level(kind: &StmtKind, span: Span, file: &Path) -> Result<(), LoadError> {
    match kind {
        StmtKind::Fn { .. }
        | StmtKind::Enum { .. }
        | StmtKind::Tool { .. }
        | StmtKind::Infer { .. }
        | StmtKind::Import(_) => Ok(()),
        StmtKind::ImportFile { .. } => unreachable!("handled by its own match arm in resolve_file"),
        _ => Err(LoadError::IllegalTopLevelStatement {
            file: file.display().to_string(),
            span,
        }),
    }
}

/// Builds `old name -> alias_old_name` for every `fn`/`tool`/`infer`
/// name and every `enum` name (plus its flattened `EnumName_Variant`
/// identifier forms) declared at `stmts`' top level. Applied to the
/// *whole* list afterward, so a name arriving from an already-resolved
/// sub-import (itself already alias-prefixed) gets this alias
/// prepended too — the cascading behavior SPEC.md describes.
fn collect_rename_map(stmts: &[Stmt], alias: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Fn { name, .. }
            | StmtKind::Tool { name, .. }
            | StmtKind::Infer { name, .. } => {
                map.insert(name.clone(), format!("{alias}_{name}"));
            }
            StmtKind::Enum { name, variants } => {
                map.insert(name.clone(), format!("{alias}_{name}"));
                for variant in variants {
                    map.insert(
                        format!("{name}_{variant}"),
                        format!("{alias}_{name}_{variant}"),
                    );
                }
            }
            _ => {}
        }
    }
    map
}

fn rn(name: &str, map: &std::collections::HashMap<String, String>) -> String {
    map.get(name).cloned().unwrap_or_else(|| name.to_string())
}

fn rename_type(ty: Type, map: &std::collections::HashMap<String, String>) -> Type {
    match ty {
        Type::Enum(name) => Type::Enum(rn(&name, map)),
        Type::List(inner) => Type::List(Box::new(rename_type(*inner, map))),
        Type::Option(inner) => Type::Option(Box::new(rename_type(*inner, map))),
        Type::Task(inner) => Type::Task(Box::new(rename_type(*inner, map))),
        Type::Inference(inner) => Type::Inference(Box::new(rename_type(*inner, map))),
        Type::Distribution(inner) => Type::Distribution(Box::new(rename_type(*inner, map))),
        Type::Tool(inner) => Type::Tool(Box::new(rename_type(*inner, map))),
        other @ (Type::Int | Type::Float | Type::Bool | Type::String | Type::Unit) => other,
    }
}

fn rename_params(
    params: Vec<Param>,
    map: &std::collections::HashMap<String, String>,
) -> Vec<Param> {
    params
        .into_iter()
        .map(|p| Param {
            name: p.name,
            ty: rename_type(p.ty, map),
        })
        .collect()
}

fn rename_expr(expr: Expr, map: &std::collections::HashMap<String, String>) -> Expr {
    let kind = match expr.kind {
        ExprKind::Identifier(name) => ExprKind::Identifier(rn(&name, map)),
        ExprKind::Unary { op, operand } => ExprKind::Unary {
            op,
            operand: Box::new(rename_expr(*operand, map)),
        },
        ExprKind::Binary { op, left, right } => ExprKind::Binary {
            op,
            left: Box::new(rename_expr(*left, map)),
            right: Box::new(rename_expr(*right, map)),
        },
        ExprKind::Call { callee, args } => ExprKind::Call {
            callee: Box::new(rename_expr(*callee, map)),
            args: args.into_iter().map(|a| rename_expr(a, map)).collect(),
        },
        ExprKind::List(items) => {
            ExprKind::List(items.into_iter().map(|i| rename_expr(i, map)).collect())
        }
        ExprKind::Index { object, index } => ExprKind::Index {
            object: Box::new(rename_expr(*object, map)),
            index: Box::new(rename_expr(*index, map)),
        },
        ExprKind::Await(inner) => ExprKind::Await(Box::new(rename_expr(*inner, map))),
        other @ (ExprKind::Integer(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Bool(_)) => other,
    };
    Expr::new(kind, expr.span)
}

fn rename_block(block: Block, map: &std::collections::HashMap<String, String>) -> Block {
    Block {
        statements: block
            .statements
            .into_iter()
            .map(|s| rename_stmt(s, map))
            .collect(),
        span: block.span,
    }
}

fn rename_stmt(stmt: Stmt, map: &std::collections::HashMap<String, String>) -> Stmt {
    let kind = match stmt.kind {
        StmtKind::Let { name, value } => StmtKind::Let {
            name,
            value: rename_expr(value, map),
        },
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => StmtKind::If {
            condition: rename_expr(condition, map),
            then_branch: rename_block(then_branch, map),
            else_branch: else_branch.map(|b| rename_block(b, map)),
        },
        StmtKind::Expr(e) => StmtKind::Expr(rename_expr(e, map)),
        StmtKind::Fn {
            name,
            params,
            return_type,
            body,
            is_async,
            effects,
        } => StmtKind::Fn {
            name: rn(&name, map),
            params: rename_params(params, map),
            return_type: rename_type(return_type, map),
            body: rename_block(body, map),
            is_async,
            effects,
        },
        StmtKind::Return(e) => StmtKind::Return(rename_expr(e, map)),
        StmtKind::Import(m) => StmtKind::Import(m),
        StmtKind::ImportFile { path, alias } => StmtKind::ImportFile { path, alias },
        StmtKind::Infer {
            name,
            params,
            return_type,
            permissions,
        } => StmtKind::Infer {
            name: rn(&name, map),
            params: rename_params(params, map),
            return_type: rename_type(return_type, map),
            permissions: permissions.map(|list| list.into_iter().map(|t| rn(&t, map)).collect()),
        },
        StmtKind::Enum { name, variants } => StmtKind::Enum {
            name: rn(&name, map),
            variants,
        },
        StmtKind::Tool {
            name,
            params,
            return_type,
        } => StmtKind::Tool {
            name: rn(&name, map),
            params: rename_params(params, map),
            return_type: rename_type(return_type, map),
        },
        StmtKind::Test { name, body } => StmtKind::Test {
            name,
            body: rename_block(body, map),
        },
        StmtKind::Mock { function, value } => StmtKind::Mock {
            function: rn(&function, map),
            value: rename_expr(value, map),
        },
        StmtKind::Assert { condition } => StmtKind::Assert {
            condition: rename_expr(condition, map),
        },
        StmtKind::Budget { .. } => stmt.kind,
    };
    Stmt::new(kind, stmt.span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("aint_loader_test_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("should create the workspace root");
            Self { root }
        }

        fn write(&self, rel_path: &str, source: &str) -> PathBuf {
            let path = self.root.join(rel_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("should create parent dir");
            }
            fs::write(&path, source).expect("should write source");
            path
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_single_file_program_is_unchanged() {
        let ws = TempWorkspace::new("single_file");
        let entry = ws.write("main.an", "print(\"hi\")\n");
        let program = load(&entry).expect("should load");
        assert_eq!(program.statements.len(), 1);
    }

    #[test]
    fn imported_declarations_are_renamed_and_spliced_in() {
        let ws = TempWorkspace::new("basic_import");
        ws.write(
            "util.an",
            "fn greet(name: String) -> String effects [pure] {\n    return name\n}\n",
        );
        let entry = ws.write(
            "main.an",
            "import \"./util.an\" as util\n\nfn main() -> String {\n    return util_greet(\"world\")\n}\n",
        );

        let program = load(&entry).expect("should load");
        let names: Vec<String> = program
            .statements
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Fn { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["util_greet".to_string(), "main".to_string()]);
    }

    #[test]
    fn enum_variant_identifiers_are_renamed() {
        let ws = TempWorkspace::new("enum_variant");
        ws.write(
            "kinds.an",
            "enum Status { Active Done }\n\nfn is_done(s: Status) -> Bool {\n    return s == Status_Done\n}\n",
        );
        let entry = ws.write("main.an", "import \"./kinds.an\" as kinds\n");

        let program = load(&entry).expect("should load");
        let is_done = program
            .statements
            .iter()
            .find_map(|s| match &s.kind {
                StmtKind::Fn { name, body, .. } if name == "kinds_is_done" => Some(body.clone()),
                _ => None,
            })
            .expect("kinds_is_done should exist, renamed");

        let source = format!("{is_done:?}");
        assert!(source.contains("kinds_Status_Done"));
    }

    #[test]
    fn a_direct_cycle_is_rejected() {
        let ws = TempWorkspace::new("cycle");
        ws.write("a.an", "import \"./b.an\" as b\n");
        ws.write("b.an", "import \"./a.an\" as a\n");
        let entry = ws.write("entry.an", "import \"./a.an\" as a\n");

        let err = load(&entry).expect_err("should reject a cycle");
        assert!(matches!(err, LoadError::Cycle { .. }));
    }

    #[test]
    fn a_diamond_import_is_rejected_in_v1() {
        let ws = TempWorkspace::new("diamond");
        ws.write("shared.an", "fn helper() -> Int {\n    return 1\n}\n");
        ws.write("a.an", "import \"./shared.an\" as shared\n");
        ws.write("b.an", "import \"./shared.an\" as shared\n");
        let entry = ws.write(
            "entry.an",
            "import \"./a.an\" as a\nimport \"./b.an\" as b\n",
        );

        let err = load(&entry).expect_err("should reject a diamond import");
        assert!(matches!(err, LoadError::DuplicateImport { .. }));
    }

    #[test]
    fn two_imports_sharing_an_alias_are_rejected() {
        let ws = TempWorkspace::new("dup_alias");
        ws.write("a.an", "fn a_fn() -> Int {\n    return 1\n}\n");
        ws.write("b.an", "fn b_fn() -> Int {\n    return 2\n}\n");
        let entry = ws.write(
            "entry.an",
            "import \"./a.an\" as shared\nimport \"./b.an\" as shared\n",
        );

        let err = load(&entry).expect_err("should reject a duplicate alias");
        assert!(matches!(err, LoadError::DuplicateAlias { .. }));
    }

    #[test]
    fn a_let_at_an_imported_files_top_level_is_rejected() {
        let ws = TempWorkspace::new("illegal_top_level");
        ws.write("consts.an", "let x = 1\n");
        let entry = ws.write("entry.an", "import \"./consts.an\" as consts\n");

        let err = load(&entry).expect_err("should reject a top-level let in an imported file");
        assert!(matches!(err, LoadError::IllegalTopLevelStatement { .. }));
    }

    #[test]
    fn a_missing_import_path_is_a_clear_io_error() {
        let ws = TempWorkspace::new("missing_file");
        let entry = ws.write("entry.an", "import \"./nope.an\" as nope\n");

        let err = load(&entry).expect_err("should fail to resolve");
        assert!(matches!(err, LoadError::Io { .. }));
    }
}
