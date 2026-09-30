//! Anki-compatible TSV import.
//!
//! The whole file is imported in a single transaction: if any line is
//! malformed, or a database operation fails, the transaction is rolled back
//! and the database is left untouched. Leading `#` directive lines and blank
//! lines are ignored. Each remaining line must have exactly three
//! tab-separated columns: `front`, `back`, `group`. An empty group means the
//! configured default group; a group that does not exist yet is created.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::db::{self, cards, groups};

/// Errors produced while importing.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// A database error.
    #[error(transparent)]
    Db(#[from] db::DbError),
    /// A filesystem error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// A line did not have exactly three tab-separated columns.
    #[error("line {line}: expected 3 tab-separated columns, found {found}")]
    BadColumnCount {
        /// 1-based line number in the file.
        line: usize,
        /// Number of columns actually found.
        found: usize,
    },
}

/// Convenience alias for import results.
pub type Result<T> = std::result::Result<T, ImportError>;

/// What an import did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Number of cards inserted.
    pub cards: usize,
    /// Number of groups created.
    pub groups_created: usize,
}

/// Import `path`, creating missing groups and using `default_group` for rows
/// without a group name.
pub fn import_tsv(conn: &Connection, path: &Path, default_group: &str) -> Result<ImportSummary> {
    let text = std::fs::read_to_string(path)?;
    let default_group = default_group.trim();

    let tx = conn.unchecked_transaction().map_err(db::DbError::from)?;

    let mut group_ids: HashMap<String, i64> = groups::list(&tx)?
        .into_iter()
        .map(|group| (group.name, group.id))
        .collect();
    let mut summary = ImportSummary::default();

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() != 3 {
            return Err(ImportError::BadColumnCount {
                line: line_number,
                found: columns.len(),
            });
        }

        let front = columns[0];
        let back = columns[1];
        let name = columns[2].trim();
        let group_name = if name.is_empty() {
            default_group.to_string()
        } else {
            name.to_string()
        };

        let group_id = match group_ids.get(&group_name) {
            Some(id) => *id,
            None => {
                let group = groups::create(&tx, &group_name)?;
                group_ids.insert(group.name.clone(), group.id);
                summary.groups_created += 1;
                group.id
            }
        };

        cards::create(&tx, front, back, group_id)?;
        summary.cards += 1;
    }

    tx.commit().map_err(db::DbError::from)?;
    Ok(summary)
}
