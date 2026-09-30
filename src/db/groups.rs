//! Group ("dictionary") operations.

use rusqlite::{params, Connection, OptionalExtension, Row};

use super::models::Group;
use super::{duplicate_name_error, now, timed, DbError, Result};

fn row_to_group(row: &Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get("id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
    })
}

/// Whether a group with this id exists.
pub fn exists(conn: &Connection, id: i64) -> Result<bool> {
    timed("groups::exists", || {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM groups WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    })
}

/// Create a group with `name` (trimmed, must be unique).
pub fn create(conn: &Connection, name: &str) -> Result<Group> {
    let name = name.trim();
    if name.is_empty() {
        return Err(DbError::EmptyGroupName);
    }
    let created_at = now();
    let result = timed("groups::create", || {
        conn.execute(
            "INSERT INTO groups (name, created_at) VALUES (?1, ?2)",
            params![name, created_at],
        )?;
        Ok(conn.last_insert_rowid())
    });
    match result {
        Ok(id) => Ok(Group {
            id,
            name: name.to_string(),
            created_at,
        }),
        Err(error) => Err(duplicate_name_error(error, name)),
    }
}

/// Rename a group. Fails on an empty or duplicate name.
pub fn rename(conn: &Connection, id: i64, new_name: &str) -> Result<()> {
    let name = new_name.trim();
    if name.is_empty() {
        return Err(DbError::EmptyGroupName);
    }
    let result = timed("groups::rename", || {
        let changed = conn.execute(
            "UPDATE groups SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        if changed == 0 {
            return Err(DbError::GroupNotFound(id));
        }
        Ok(())
    });
    result.map_err(|error| duplicate_name_error(error, name))
}

/// Delete a group.
///
/// Refuses to delete a group that still contains cards or the last remaining
/// group, so the database always has at least one group to attach cards to.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    timed("groups::delete", || {
        if !exists(conn, id)? {
            return Err(DbError::GroupNotFound(id));
        }

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM groups", [], |row| row.get(0))?;
        if total <= 1 {
            return Err(DbError::LastGroup);
        }

        let cards: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE group_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        if cards > 0 {
            return Err(DbError::GroupNotEmpty(id));
        }

        conn.execute("DELETE FROM groups WHERE id = ?1", params![id])?;
        Ok(())
    })
}

/// All groups, ordered by name.
pub fn list(conn: &Connection) -> Result<Vec<Group>> {
    timed("groups::list", || {
        let mut stmt = conn.prepare("SELECT id, name, created_at FROM groups ORDER BY name")?;
        let rows = stmt.query_map([], row_to_group)?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    })
}

/// Fetch a single group.
pub fn get(conn: &Connection, id: i64) -> Result<Group> {
    timed("groups::get", || {
        conn.query_row(
            "SELECT id, name, created_at FROM groups WHERE id = ?1",
            params![id],
            row_to_group,
        )
        .optional()?
        .ok_or(DbError::GroupNotFound(id))
    })
}

/// Find a group by exact name.
pub fn find_by_name(conn: &Connection, name: &str) -> Result<Option<Group>> {
    timed("groups::find_by_name", || {
        Ok(conn
            .query_row(
                "SELECT id, name, created_at FROM groups WHERE name = ?1",
                params![name],
                row_to_group,
            )
            .optional()?)
    })
}

/// Number of cards in a group.
pub fn card_count(conn: &Connection, group_id: i64) -> Result<i64> {
    timed("groups::card_count", || {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM cards WHERE group_id = ?1",
            params![group_id],
            |row| row.get(0),
        )?)
    })
}
