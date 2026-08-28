//! The file-backed store behind the `db` stdlib module (milestone
//! 25). Each table is one newline-delimited-JSON file under
//! `.aintdb/<table>.jsonl`, one `Row` per line. `record` holds the
//! caller's JSON string opaquely — this layer never parses its
//! *contents*, only the wrapper, so it never needs to understand what
//! shape the caller's data is. See
//! `docs/milestones/25-real-application/SPEC.md` for why a hand-rolled
//! JSONL file, not a real embedded database engine.

use std::fs;
use std::path::{Path, PathBuf};

use aint_ast::Span;
use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

/// `.aintdb`, relative to the current working directory — what every
/// real call from `stdlib::call` uses. Tests pass a scratch temp
/// directory directly instead, since `db`'s own functions take the
/// base directory as a parameter rather than reading process-global
/// state (`std::env::set_current_dir` is process-wide and would race
/// across `cargo test`'s default parallel test threads).
pub(crate) const DEFAULT_DB_DIR: &str = ".aintdb";

#[derive(Serialize, Deserialize)]
struct Row {
    id: String,
    record: String,
}

fn table_path(base_dir: &Path, table: &str) -> PathBuf {
    base_dir.join(format!("{table}.jsonl"))
}

fn read_table(base_dir: &Path, table: &str, span: Span) -> Result<Vec<Row>, RuntimeError> {
    let path = table_path(base_dir, table);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(|err| RuntimeError::Io {
        message: format!("could not read {}: {err}", path.display()),
        span,
    })?;
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|err| RuntimeError::Io {
                message: format!("{} is corrupt: {err}", path.display()),
                span,
            })
        })
        .collect()
}

fn write_table(base_dir: &Path, table: &str, rows: &[Row], span: Span) -> Result<(), RuntimeError> {
    fs::create_dir_all(base_dir).map_err(|err| RuntimeError::Io {
        message: format!("could not create {}: {err}", base_dir.display()),
        span,
    })?;
    let path = table_path(base_dir, table);
    let mut content = String::new();
    for row in rows {
        content.push_str(&serde_json::to_string(row).expect("Row always serializes"));
        content.push('\n');
    }
    fs::write(&path, content).map_err(|err| RuntimeError::Io {
        message: format!("could not write {}: {err}", path.display()),
        span,
    })
}

/// `false` if `id` already exists in `table` — matches "insert," not
/// "upsert"; see `update` for overwriting.
pub(crate) fn insert(
    base_dir: &Path,
    table: &str,
    id: &str,
    record: &str,
    span: Span,
) -> Result<bool, RuntimeError> {
    let mut rows = read_table(base_dir, table, span)?;
    if rows.iter().any(|row| row.id == id) {
        return Ok(false);
    }
    rows.push(Row {
        id: id.to_string(),
        record: record.to_string(),
    });
    write_table(base_dir, table, &rows, span)?;
    Ok(true)
}

pub(crate) fn get(
    base_dir: &Path,
    table: &str,
    id: &str,
    span: Span,
) -> Result<Option<String>, RuntimeError> {
    let rows = read_table(base_dir, table, span)?;
    Ok(rows
        .into_iter()
        .find(|row| row.id == id)
        .map(|row| row.record))
}

pub(crate) fn list(base_dir: &Path, table: &str, span: Span) -> Result<Vec<String>, RuntimeError> {
    let rows = read_table(base_dir, table, span)?;
    Ok(rows.into_iter().map(|row| row.record).collect())
}

pub(crate) fn update(
    base_dir: &Path,
    table: &str,
    id: &str,
    record: &str,
    span: Span,
) -> Result<bool, RuntimeError> {
    let mut rows = read_table(base_dir, table, span)?;
    match rows.iter_mut().find(|row| row.id == id) {
        Some(row) => {
            row.record = record.to_string();
            write_table(base_dir, table, &rows, span)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub(crate) fn delete(
    base_dir: &Path,
    table: &str,
    id: &str,
    span: Span,
) -> Result<bool, RuntimeError> {
    let mut rows = read_table(base_dir, table, span)?;
    let original_len = rows.len();
    rows.retain(|row| row.id != id);
    if rows.len() == original_len {
        return Ok(false);
    }
    write_table(base_dir, table, &rows, span)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aint_ast::Position;

    fn span() -> Span {
        Span::new(Position::start(), Position::start())
    }

    /// A fresh scratch directory for one test, cleaned up on drop.
    /// `db`'s functions take this as an explicit parameter rather
    /// than reading `std::env::current_dir` internally specifically
    /// so tests can do this instead of `set_current_dir` (process-
    /// global, and would race across parallel test threads).
    struct ScratchDir {
        path: PathBuf,
    }

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("aint_db_test_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("should create scratch dir");
            Self { path }
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn insert_then_get_round_trips() {
        let scratch = ScratchDir::new("insert_get");
        assert!(insert(
            &scratch.path,
            "tickets",
            "1",
            "{\"subject\":\"help\"}",
            span()
        )
        .unwrap());
        assert_eq!(
            get(&scratch.path, "tickets", "1", span()).unwrap(),
            Some("{\"subject\":\"help\"}".to_string())
        );
    }

    #[test]
    fn get_of_a_missing_id_is_none() {
        let scratch = ScratchDir::new("get_missing");
        assert_eq!(
            get(&scratch.path, "tickets", "nonexistent", span()).unwrap(),
            None
        );
    }

    #[test]
    fn insert_of_a_duplicate_id_fails() {
        let scratch = ScratchDir::new("insert_dup");
        assert!(insert(&scratch.path, "tickets", "1", "a", span()).unwrap());
        assert!(!insert(&scratch.path, "tickets", "1", "b", span()).unwrap());
        assert_eq!(
            get(&scratch.path, "tickets", "1", span()).unwrap(),
            Some("a".to_string())
        );
    }

    #[test]
    fn update_replaces_an_existing_record() {
        let scratch = ScratchDir::new("update");
        insert(&scratch.path, "tickets", "1", "old", span()).unwrap();
        assert!(update(&scratch.path, "tickets", "1", "new", span()).unwrap());
        assert_eq!(
            get(&scratch.path, "tickets", "1", span()).unwrap(),
            Some("new".to_string())
        );
    }

    #[test]
    fn update_of_a_missing_id_returns_false() {
        let scratch = ScratchDir::new("update_missing");
        assert!(!update(&scratch.path, "tickets", "nonexistent", "x", span()).unwrap());
    }

    #[test]
    fn delete_removes_a_record() {
        let scratch = ScratchDir::new("delete");
        insert(&scratch.path, "tickets", "1", "a", span()).unwrap();
        assert!(delete(&scratch.path, "tickets", "1", span()).unwrap());
        assert_eq!(get(&scratch.path, "tickets", "1", span()).unwrap(), None);
    }

    #[test]
    fn delete_of_a_missing_id_returns_false() {
        let scratch = ScratchDir::new("delete_missing");
        assert!(!delete(&scratch.path, "tickets", "nonexistent", span()).unwrap());
    }

    #[test]
    fn list_returns_every_record_in_a_table() {
        let scratch = ScratchDir::new("list");
        insert(&scratch.path, "tickets", "1", "a", span()).unwrap();
        insert(&scratch.path, "tickets", "2", "b", span()).unwrap();
        let mut records = list(&scratch.path, "tickets", span()).unwrap();
        records.sort();
        assert_eq!(records, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn list_of_an_empty_table_is_an_empty_list() {
        let scratch = ScratchDir::new("list_empty");
        assert_eq!(
            list(&scratch.path, "nonexistent_table", span()).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn separate_tables_do_not_interfere() {
        let scratch = ScratchDir::new("separate_tables");
        insert(&scratch.path, "tickets", "1", "a-ticket", span()).unwrap();
        insert(&scratch.path, "users", "1", "a-user", span()).unwrap();
        assert_eq!(
            get(&scratch.path, "tickets", "1", span()).unwrap(),
            Some("a-ticket".to_string())
        );
        assert_eq!(
            get(&scratch.path, "users", "1", span()).unwrap(),
            Some("a-user".to_string())
        );
    }
}
