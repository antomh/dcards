//! Tests for Anki-TSV export and import.

use std::path::Path;

use dcards::db::{self, cards, groups, ExportRow};
use dcards::import::{ImportError, ImportSummary};
use dcards::{export, import};
use rusqlite::Connection;

fn setup() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("dcards.db")).unwrap();
    db::migrate(&conn).unwrap();
    db::bootstrap_default_group(&conn, "General").unwrap();
    (dir, conn)
}

/// All cards as `(front, back, group)`, sorted for comparison.
fn rows(conn: &Connection) -> Vec<ExportRow> {
    let mut rows = cards::export_rows(conn, None).unwrap();
    rows.sort_by(|a, b| (&a.front, &a.back, &a.group).cmp(&(&b.front, &b.back, &b.group)));
    rows
}

fn count_cards(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap()
}

fn write_file(dir: &Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn export_writes_header_and_sanitized_single_line_rows() {
    let (dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    cards::create(&conn, "line1\nline2", "a\tb", group.id).unwrap();

    let path = dir.path().join("group.tsv");
    let written = export::export_group(&conn, group.id, &path).unwrap();
    assert_eq!(written, 1);

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("#separator:Tab\n#html:false\n"));

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "two directive lines and one card line");

    let columns: Vec<&str> = lines[2].split('\t').collect();
    assert_eq!(columns.len(), 3);
    assert_eq!(columns[0], "line1 line2");
    assert_eq!(columns[1], "a b");
    assert_eq!(columns[2], "G");
}

#[test]
fn round_trip_preserves_cards_and_creates_groups() {
    let (dir, source) = setup();
    let travel = groups::create(&source, "Travel").unwrap();
    let food = groups::create(&source, "Food").unwrap();
    cards::create(&source, "house", "дом", travel.id).unwrap();
    cards::create(&source, "cat", "кошка", travel.id).unwrap();
    cards::create(&source, "bread", "хлеб", food.id).unwrap();

    let path = dir.path().join("all.tsv");
    let written = export::export_all(&source, &path).unwrap();
    assert_eq!(written, 3);

    let (_dir2, target) = setup();
    let summary: ImportSummary = import::import_tsv(&target, &path, "General").unwrap();
    assert_eq!(summary.cards, 3);
    assert_eq!(summary.groups_created, 2);

    assert_eq!(rows(&source), rows(&target));
}

#[test]
fn import_creates_missing_groups() {
    let (dir, conn) = setup();
    let path = write_file(
        dir.path(),
        "in.tsv",
        "#separator:Tab\n#html:false\nword\tслово\tTravel\n",
    );

    let summary = import::import_tsv(&conn, &path, "General").unwrap();
    assert_eq!(summary.cards, 1);
    assert_eq!(summary.groups_created, 1);

    let travel = groups::find_by_name(&conn, "Travel").unwrap().unwrap();
    let card = cards::list_by_group(&conn, travel.id, &db::CardFilter::default(), 10).unwrap();
    assert_eq!(card.len(), 1);
    assert_eq!(card[0].front, "word");
    assert_eq!(card[0].back, "слово");
}

#[test]
fn empty_group_column_uses_the_default_group() {
    let (dir, conn) = setup();
    let path = write_file(dir.path(), "in.tsv", "word\tслово\t\n");

    let summary = import::import_tsv(&conn, &path, "General").unwrap();
    assert_eq!(summary.cards, 1);
    assert_eq!(summary.groups_created, 0);

    let general = groups::find_by_name(&conn, "General").unwrap().unwrap();
    let card = cards::list_by_group(&conn, general.id, &db::CardFilter::default(), 10).unwrap();
    assert_eq!(card.len(), 1);
}

#[test]
fn a_broken_line_rolls_the_whole_import_back() {
    let (dir, conn) = setup();
    let path = write_file(
        dir.path(),
        "in.tsv",
        "#separator:Tab\n#html:false\nok\tfine\tGroup1\nbroken\trow\n",
    );

    let error = import::import_tsv(&conn, &path, "General").unwrap_err();
    assert!(matches!(
        error,
        ImportError::BadColumnCount { line: 4, found: 2 }
    ));

    // Nothing was committed: no cards and only the bootstrapped default group.
    assert_eq!(count_cards(&conn), 0);
    assert_eq!(groups::list(&conn).unwrap().len(), 1);
}
