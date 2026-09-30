//! Tests for review session construction.

use dcards::db::{self, cards, groups, CardFilter};
use dcards::review;
use rusqlite::Connection;

fn setup() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("dcards.db")).unwrap();
    db::migrate(&conn).unwrap();
    (dir, conn)
}

/// Insert a card with an explicit `created_at` and return its id.
fn insert_card(conn: &Connection, group_id: i64, front: &str, created_at: i64) -> i64 {
    conn.execute(
        "INSERT INTO cards (front, back, group_id, created_at, updated_at)
         VALUES (?1, '', ?2, ?3, ?3)",
        rusqlite::params![front, group_id, created_at],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn ordered_ids(conn: &Connection, group_id: i64, limit: usize) -> Vec<i64> {
    cards::review_query(conn, group_id, &CardFilter::default(), limit)
        .unwrap()
        .iter()
        .map(|card| card.id)
        .collect()
}

#[test]
fn session_respects_limit_and_selection() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    for i in 0..10 {
        insert_card(&conn, group.id, &format!("f{i}"), i);
    }

    let session = review::build_session(&conn, group.id, &CardFilter::default(), 4, 42).unwrap();
    assert_eq!(session.len(), 4, "must not exceed the limit");

    let mut got: Vec<i64> = session.cards().iter().map(|card| card.id).collect();
    let mut expected = ordered_ids(&conn, group.id, 4);
    assert_eq!(got.len(), expected.len());

    // Same selection, different order.
    assert_ne!(
        session.cards().iter().map(|c| c.id).collect::<Vec<_>>(),
        expected,
        "the session should be shuffled"
    );

    got.sort_unstable();
    expected.sort_unstable();
    assert_eq!(got, expected, "same composition as the query");
}

#[test]
fn session_applies_the_date_filter() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    for timestamp in [100_i64, 200, 300, 400, 500] {
        insert_card(&conn, group.id, &format!("t{timestamp}"), timestamp);
    }

    let filter = CardFilter {
        group_id: None,
        from: Some(200),
        to: Some(400),
    };
    let session = review::build_session(&conn, group.id, &filter, 100, 7).unwrap();

    assert_eq!(session.len(), 3);
    let mut ids: Vec<i64> = session.cards().iter().map(|card| card.id).collect();
    ids.sort_unstable();
    let mut expected = ordered_ids_filtered(&conn, group.id, &filter);
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

fn ordered_ids_filtered(conn: &Connection, group_id: i64, filter: &CardFilter) -> Vec<i64> {
    cards::review_query(conn, group_id, filter, 100)
        .unwrap()
        .iter()
        .map(|card| card.id)
        .collect()
}

#[test]
fn session_navigation() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "G").unwrap();
    insert_card(&conn, group.id, "one", 1);
    insert_card(&conn, group.id, "two", 2);

    let mut session =
        review::build_session(&conn, group.id, &CardFilter::default(), 10, 1).unwrap();
    assert_eq!(session.len(), 2);
    assert!(!session.is_empty());
    assert_eq!(session.position(), 1);
    assert!(!session.is_revealed());
    assert!(session.current().is_some());
    assert!(!session.is_finished());

    session.reveal();
    assert!(session.is_revealed());

    session.advance();
    assert_eq!(session.position(), 2);
    assert!(!session.is_revealed());
    assert!(!session.is_finished());

    session.advance();
    assert!(session.is_finished());
    assert!(session.current().is_none());
}

#[test]
fn empty_group_yields_an_empty_session() {
    let (_dir, conn) = setup();
    let group = groups::create(&conn, "Empty").unwrap();

    let session = review::build_session(&conn, group.id, &CardFilter::default(), 10, 1).unwrap();
    assert!(session.is_empty());
    assert_eq!(session.len(), 0);
    assert!(session.is_finished());
}
