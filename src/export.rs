//! Anki-compatible TSV export.
//!
//! The file starts with two directive lines (`#separator:Tab`, `#html:false`),
//! followed by one card per line as `front<TAB>back<TAB>group`. Tabs, newlines
//! and carriage returns inside a field are replaced with spaces so every
//! record stays on a single line.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use rusqlite::Connection;

use crate::db::{self, cards, ExportRow};

/// Directive lines Anki expects at the top of a text file.
const HEADER: &str = "#separator:Tab\n#html:false\n";

/// Errors produced while exporting.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// A database error.
    #[error(transparent)]
    Db(#[from] db::DbError),
    /// A filesystem error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Convenience alias for export results.
pub type Result<T> = std::result::Result<T, ExportError>;

/// Export every card of `group_id` to `path`. Returns the number of cards.
pub fn export_group(conn: &Connection, group_id: i64, path: &Path) -> Result<usize> {
    let rows = cards::export_rows(conn, Some(group_id))?;
    write_tsv(path, &rows)
}

/// Export every card in the database to `path`. Returns the number of cards.
pub fn export_all(conn: &Connection, path: &Path) -> Result<usize> {
    let rows = cards::export_rows(conn, None)?;
    write_tsv(path, &rows)
}

/// Render the header and rows as TSV text.
pub fn format_tsv(rows: &[ExportRow]) -> String {
    let mut out = String::from(HEADER);
    for row in rows {
        out.push_str(&sanitize(&row.front));
        out.push('\t');
        out.push_str(&sanitize(&row.back));
        out.push('\t');
        out.push_str(&sanitize(&row.group));
        out.push('\n');
    }
    out
}

fn write_tsv(path: &Path, rows: &[ExportRow]) -> Result<usize> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(format_tsv(rows).as_bytes())?;
    writer.flush()?;
    Ok(rows.len())
}

/// Replace characters that would break the single-line TSV format.
pub fn sanitize(field: &str) -> String {
    field
        .chars()
        .map(|ch| match ch {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}
