//! Card operations.

use rusqlite::{params, Connection, OptionalExtension, Row};

use super::groups;
use super::models::{Card, CardFilter, ExportRow};
use super::{now, timed, DbError, Result};

/// Ordering of card query results.
#[derive(Debug, Clone, Copy)]
enum Order {
    /// Newest first; used by the group view.
    CreatedDesc,
    /// Insertion order; used by the review session before shuffling.
    IdAsc,
}

impl Order {
    fn sql(self) -> &'static str {
        match self {
            Order::CreatedDesc => "created_at DESC, id DESC",
            Order::IdAsc => "id ASC",
        }
    }
}

fn row_to_card(row: &Row<'_>) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get("id")?,
        front: row.get("front")?,
        back: row.get("back")?,
        group_id: row.get("group_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Create a card in `group_id`.
///
/// Fails with [`DbError::GroupNotFound`] when the group does not exist.
pub fn create(conn: &Connection, front: &str, back: &str, group_id: i64) -> Result<Card> {
    if !groups::exists(conn, group_id)? {
        return Err(DbError::GroupNotFound(group_id));
    }
    let timestamp = now();
    timed("cards::create", || {
        conn.execute(
            "INSERT INTO cards (front, back, group_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![front, back, group_id, timestamp],
        )?;
        Ok(Card {
            id: conn.last_insert_rowid(),
            front: front.to_string(),
            back: back.to_string(),
            group_id,
            created_at: timestamp,
            updated_at: timestamp,
        })
    })
}

/// Update a card's content and group, refreshing `updated_at`.
pub fn update(conn: &Connection, id: i64, front: &str, back: &str, group_id: i64) -> Result<()> {
    if !groups::exists(conn, group_id)? {
        return Err(DbError::GroupNotFound(group_id));
    }
    let timestamp = now();
    let changed = timed("cards::update", || {
        Ok(conn.execute(
            "UPDATE cards
             SET front = ?1, back = ?2, group_id = ?3, updated_at = ?4
             WHERE id = ?5",
            params![front, back, group_id, timestamp, id],
        )?)
    })?;
    if changed == 0 {
        return Err(DbError::CardNotFound(id));
    }
    Ok(())
}

/// Delete a card.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let changed = timed("cards::delete", || {
        Ok(conn.execute("DELETE FROM cards WHERE id = ?1", params![id])?)
    })?;
    if changed == 0 {
        return Err(DbError::CardNotFound(id));
    }
    Ok(())
}

/// Fetch a single card.
pub fn get(conn: &Connection, id: i64) -> Result<Card> {
    timed("cards::get", || {
        conn.query_row(
            "SELECT id, front, back, group_id, created_at, updated_at
             FROM cards WHERE id = ?1",
            params![id],
            row_to_card,
        )
        .optional()?
        .ok_or(DbError::CardNotFound(id))
    })
}

/// Cards of a group, newest first, honouring optional date bounds and a limit.
pub fn list_by_group(
    conn: &Connection,
    group_id: i64,
    filter: &CardFilter,
    limit: usize,
) -> Result<Vec<Card>> {
    let mut effective = filter.clone();
    effective.group_id = Some(group_id);
    query(
        conn,
        &effective,
        limit,
        Order::CreatedDesc,
        "cards::list_by_group",
    )
}

/// Cards of a group for a review session.
///
/// The result is *not* shuffled here; the caller shuffles it. Insertion order
/// is used so the query itself is deterministic.
pub fn review_query(
    conn: &Connection,
    group_id: i64,
    filter: &CardFilter,
    limit: usize,
) -> Result<Vec<Card>> {
    let mut effective = filter.clone();
    effective.group_id = Some(group_id);
    query(conn, &effective, limit, Order::IdAsc, "cards::review_query")
}

/// Number of cards in a group.
pub fn count_by_group(conn: &Connection, group_id: i64) -> Result<i64> {
    timed("cards::count_by_group", || {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE group_id = ?1",
            params![group_id],
            |row| row.get(0),
        )?)
    })
}

/// Cards joined with their group names, optionally restricted to one group.
///
/// Used by the TSV export; ordered by group name then insertion order.
pub fn export_rows(conn: &Connection, group_id: Option<i64>) -> Result<Vec<ExportRow>> {
    timed("cards::export_rows", || {
        let mut stmt = conn.prepare(
            "SELECT cards.front, cards.back, groups.name
             FROM cards
             JOIN groups ON groups.id = cards.group_id
             WHERE (?1 IS NULL OR cards.group_id = ?1)
             ORDER BY groups.name COLLATE NOCASE, cards.id",
        )?;
        let rows = stmt.query_map(params![group_id], |row| {
            Ok(ExportRow {
                front: row.get(0)?,
                back: row.get(1)?,
                group: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
}

fn query(
    conn: &Connection,
    filter: &CardFilter,
    limit: usize,
    order: Order,
    name: &str,
) -> Result<Vec<Card>> {
    let sql = format!(
        "SELECT id, front, back, group_id, created_at, updated_at
         FROM cards
         WHERE (?1 IS NULL OR group_id = ?1)
           AND (?2 IS NULL OR created_at >= ?2)
           AND (?3 IS NULL OR created_at <= ?3)
         ORDER BY {}
         LIMIT ?4",
        order.sql()
    );
    timed(name, || {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![filter.group_id, filter.from, filter.to, limit as i64],
            row_to_card,
        )?;
        let mut cards = Vec::new();
        for row in rows {
            cards.push(row?);
        }
        Ok(cards)
    })
}
