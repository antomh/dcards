//! Tests for the SQLite persistence layer.

use dcards::db::{self, cards, groups, CardFilter, DbError};
use rusqlite::Connection;

/// Open a fresh database in a temporary directory and create the schema.
fn setup() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("dcards.db")).unwrap();
    db::migrate(&conn).unwrap();
    (dir, conn)
}

#[test]
fn migration_is_idempotent() {
    let (_dir, conn) = setup();
    db::migrate(&conn).unwrap();

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);

    // The schema still works afterwards.
    groups::create(&conn, "Still works").unwrap();
}

#[test]
fn null_group_id_is_rejected() {
    let (_dir, conn) = setup();
    db::bootstrap_default_group(&conn, "General").unwrap();

    let result = conn.execute(
        "INSERT INTO cards (front, back, group_id, created_at, updated_at)
         VALUES ('word', '', NULL, 0, 0)",
        [],
    );
    assert!(result.is_err(), "group_id must not be nullable");
}

#[test]
fn deleting_group_with_cards_is_rejected() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "With cards").unwrap();
    groups::create(&conn, "Other").unwrap();
    cards::create(&conn, "word", "sense", group.id).unwrap();

    assert!(matches!(
        groups::delete(&conn, group.id),
        Err(DbError::GroupNotEmpty(id)) if id == group.id
    ));

    // The foreign key enforces this at the SQL level too (ON DELETE RESTRICT).
    let raw = conn.execute(
        "DELETE FROM groups WHERE id = ?1",
        rusqlite::params![group.id],
    );
    assert!(raw.is_err(), "FK RESTRICT should reject the delete");
}

#[test]
fn deleting_the_last_group_is_rejected() {
    let (_dir, conn) = setup();
    db::bootstrap_default_group(&conn, "General").unwrap();
    let group = groups::list(&conn).unwrap().pop().unwrap();

    assert!(matches!(
        groups::delete(&conn, group.id),
        Err(DbError::LastGroup)
    ));
}

#[test]
fn deleting_an_empty_non_last_group_succeeds() {
    let (_dir, conn) = setup();
    let doomed = groups::create(&conn, "Doomed").unwrap();
    groups::create(&conn, "Survivor").unwrap();

    groups::delete(&conn, doomed.id).unwrap();

    let names: Vec<String> = groups::list(&conn)
        .unwrap()
        .into_iter()
        .map(|g| g.name)
        .collect();
    assert_eq!(names, vec!["Survivor"]);
}

#[test]
fn bootstrap_default_group_is_idempotent() {
    let (_dir, conn) = setup();

    db::bootstrap_default_group(&conn, "General").unwrap();
    db::bootstrap_default_group(&conn, "Another name").unwrap();

    let all = groups::list(&conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "General");
}

#[test]
fn group_names_are_unique_and_trimmed() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "  Travel  ").unwrap();
    assert_eq!(group.name, "Travel");

    assert!(matches!(
        groups::create(&conn, "Travel"),
        Err(DbError::DuplicateGroupName(_))
    ));
    assert!(matches!(
        groups::create(&conn, "   "),
        Err(DbError::EmptyGroupName)
    ));

    // Renaming to a free name works; to a taken name fails.
    groups::create(&conn, "Other").unwrap();
    groups::rename(&conn, group.id, "Journey").unwrap();
    assert_eq!(groups::get(&conn, group.id).unwrap().name, "Journey");
    assert!(matches!(
        groups::rename(&conn, group.id, "Other"),
        Err(DbError::DuplicateGroupName(_))
    ));
}

#[test]
fn filter_bounds_are_inclusive() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    for timestamp in [100_i64, 200, 300] {
        conn.execute(
            "INSERT INTO cards (front, back, group_id, created_at, updated_at)
             VALUES (?1, '', ?2, ?3, ?3)",
            rusqlite::params![format!("c{timestamp}"), group.id, timestamp],
        )
        .unwrap();
    }

    let wide = CardFilter {
        group_id: None,
        from: Some(200),
        to: Some(300),
    };
    let got = cards::list_by_group(&conn, group.id, &wide, 100).unwrap();
    assert_eq!(got.len(), 2);
    // Newest first.
    assert_eq!(got[0].front, "c300");
    assert_eq!(got[1].front, "c200");

    // Both bounds are inclusive: a single-point range matches exactly.
    let point = CardFilter {
        group_id: None,
        from: Some(200),
        to: Some(200),
    };
    let got = cards::list_by_group(&conn, group.id, &point, 100).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].front, "c200");

    // An open lower bound includes the earliest card.
    let from_only = CardFilter {
        group_id: None,
        from: None,
        to: Some(100),
    };
    assert_eq!(
        cards::list_by_group(&conn, group.id, &from_only, 100)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn limit_is_applied() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    for i in 0..5 {
        cards::create(&conn, &format!("f{i}"), "", group.id).unwrap();
    }

    let all = cards::list_by_group(&conn, group.id, &CardFilter::default(), 100).unwrap();
    assert_eq!(all.len(), 5);

    let limited = cards::list_by_group(&conn, group.id, &CardFilter::default(), 3).unwrap();
    assert_eq!(limited.len(), 3);
}

#[test]
fn updated_at_is_refreshed_on_update() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    let card = cards::create(&conn, "before", "old", group.id).unwrap();

    // Force a known, old timestamp so the refresh is observable.
    conn.execute(
        "UPDATE cards SET created_at = 0, updated_at = 0 WHERE id = ?1",
        rusqlite::params![card.id],
    )
    .unwrap();

    cards::update(&conn, card.id, "after", "new", group.id).unwrap();

    let updated = cards::get(&conn, card.id).unwrap();
    assert_eq!(updated.front, "after");
    assert_eq!(updated.back, "new");
    assert!(updated.updated_at > 0, "updated_at must be refreshed");
}

#[test]
fn card_requires_an_existing_group() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();

    assert!(matches!(
        cards::create(&conn, "word", "", 4242),
        Err(DbError::GroupNotFound(4242))
    ));

    let card = cards::create(&conn, "word", "", group.id).unwrap();
    assert!(matches!(
        cards::update(&conn, card.id, "word", "", 4242),
        Err(DbError::GroupNotFound(4242))
    ));
    assert!(matches!(
        cards::get(&conn, 9999),
        Err(DbError::CardNotFound(9999))
    ));
    assert!(matches!(
        cards::delete(&conn, 9999),
        Err(DbError::CardNotFound(9999))
    ));
}

#[test]
fn count_and_review_query() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    for i in 0..4 {
        cards::create(&conn, &format!("f{i}"), "", group.id).unwrap();
    }

    assert_eq!(cards::count_by_group(&conn, group.id).unwrap(), 4);
    assert_eq!(groups::card_count(&conn, group.id).unwrap(), 4);

    let review = cards::review_query(&conn, group.id, &CardFilter::default(), 2).unwrap();
    assert_eq!(review.len(), 2);
    // Insertion order, not newest-first.
    assert_eq!(review[0].front, "f0");
    assert_eq!(review[1].front, "f1");
}
