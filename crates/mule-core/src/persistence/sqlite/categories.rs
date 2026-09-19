use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::CoreError;
use crate::model::DownloadCategory;

pub fn get_categories(conn: &Connection) -> Result<Vec<DownloadCategory>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, sort_order, default_destination, default_priority, created_at, updated_at
         FROM download_categories ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(DownloadCategory {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            sort_order: row.get(3)?,
            default_destination: row.get(4)?,
            default_priority: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_category(conn: &Connection, id: &str) -> Result<Option<DownloadCategory>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, sort_order, default_destination, default_priority, created_at, updated_at
         FROM download_categories WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(DownloadCategory {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            sort_order: row.get(3)?,
            default_destination: row.get(4)?,
            default_priority: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn insert_category(conn: &mut Connection, cat: &DownloadCategory) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO download_categories (id, name, color, sort_order, default_destination, default_priority, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            cat.id,
            cat.name,
            cat.color,
            cat.sort_order,
            cat.default_destination,
            cat.default_priority,
            cat.created_at,
            cat.updated_at,
        ],
    )?;
    Ok(())
}

pub fn update_category(conn: &mut Connection, cat: &DownloadCategory) -> Result<bool, CoreError> {
    let rows = conn.execute(
        "UPDATE download_categories
         SET name = ?1, color = ?2, sort_order = ?3, default_destination = ?4, default_priority = ?5, updated_at = ?6
         WHERE id = ?7",
        params![
            cat.name,
            cat.color,
            cat.sort_order,
            cat.default_destination,
            cat.default_priority,
            cat.updated_at,
            cat.id,
        ],
    )?;
    Ok(rows > 0)
}

pub fn delete_category(conn: &mut Connection, id: &str) -> Result<usize, CoreError> {
    let tx = conn.transaction()?;
    let reassigned = tx.execute(
        "UPDATE downloads SET category_id = NULL WHERE category_id = ?1",
        params![id],
    )?;
    tx.execute("DELETE FROM download_categories WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(reassigned)
}

pub fn reorder_categories(conn: &mut Connection, category_ids: &[String]) -> Result<(), CoreError> {
    let tx = conn.transaction()?;
    {
        let mut stmt =
            tx.prepare("UPDATE download_categories SET sort_order = ?1 WHERE id = ?2")?;
        for (index, id) in category_ids.iter().enumerate() {
            stmt.execute(params![index as i32, id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn set_download_category(
    conn: &mut Connection,
    download_id: &Uuid,
    category_id: Option<&str>,
) -> Result<bool, CoreError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let rows = conn.execute(
        "UPDATE downloads SET category_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![category_id, now, download_id.to_string()],
    )?;
    Ok(rows > 0)
}
