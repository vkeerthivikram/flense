// src-tauri/src/history.rs
// SQLite storage for cleaning operations and restore tracking.
// Privacy: only stores category names and counts, not full metadata values.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::{AppError, AppResult};
use crate::types::{
    CategorySummary, HistoryEntry, HistoryStatus, MetadataCategory, OperationType,
};

// ──────────────────────────── Database Schema ────────────────────────────────

const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS cleaning_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        operation_id TEXT NOT NULL UNIQUE,
        file_path TEXT NOT NULL,
        backup_path TEXT NOT NULL,
        timestamp TEXT NOT NULL,
        hash_before TEXT NOT NULL DEFAULT '',
        hash_after TEXT,
        operation_type TEXT NOT NULL CHECK (operation_type IN ('clean', 'restore')),
        status TEXT NOT NULL CHECK (status IN ('success', 'failed', 'rolled_back')),
        error_message TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS metadata_summary (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        cleaning_history_id INTEGER NOT NULL,
        category TEXT NOT NULL,
        item_count INTEGER NOT NULL DEFAULT 0,
        removable_count INTEGER NOT NULL DEFAULT 0,
        partial_count INTEGER NOT NULL DEFAULT 0,
        read_only_count INTEGER NOT NULL DEFAULT 0,
        unsupported_count INTEGER NOT NULL DEFAULT 0,
        FOREIGN KEY (cleaning_history_id) REFERENCES cleaning_history(id)
    );
    "#,
];

// ──────────────────────────── Database Connection ────────────────────────────

/// Open or create the SQLite database.
pub fn open_db(db_path: &Path) -> AppResult<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Database(rusqlite::Error::IoError(
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        )))?;
    }

    let mut conn = Connection::open(db_path).map_err(AppError::Database)?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(AppError::Database)?;

    // Run migrations
    for migration in MIGRATIONS {
        conn.execute_batch(migration).map_err(AppError::Database)?;
    }

    Ok(conn)
}

/// Get the default database path in the app's data directory.
pub fn default_db_path() -> std::path::PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("metadata-cleaner");

    data_dir.join("history.db")
}

// ──────────────────────────── Recording Operations ───────────────────────────

/// Record a cleaning operation in the database.
pub fn record_cleaning(
    conn: &Connection,
    operation_id: &str,
    file_path: &str,
    backup_path: &str,
    hash_before: &str,
    hash_after: Option<&str>,
    metadata_summary: &[CategorySummary],
    audit_logging: bool,
) -> AppResult<i64> {
    let timestamp = Utc::now().to_rfc3339();

    let id = conn
        .call(
            |db| {
                let mut stmt = db.prepare(
                    "INSERT INTO cleaning_history 
                     (operation_id, file_path, backup_path, timestamp, hash_before, hash_after, operation_type, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'clean', 'success')",
                )?;

                stmt.insert(params![
                    operation_id,
                    file_path,
                    backup_path,
                    timestamp,
                    hash_before,
                    hash_after.unwrap_or(""),
                ])?;

                let history_id = db.last_insert_rowid();

                // Record category summaries (always stored, privacy-safe)
                for summary in metadata_summary {
                    let cat = serde_json::to_string(&summary.category)
                        .unwrap_or_else(|_| "unknown".to_string());

                    let mut cat_stmt = db.prepare(
                        "INSERT INTO metadata_summary 
                         (cleaning_history_id, category, item_count, removable_count, partial_count, read_only_count, unsupported_count)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    )?;

                    cat_stmt.insert(params![
                        history_id,
                        cat,
                        summary.item_count,
                        summary.removable_count,
                        summary.partial_count,
                        summary.read_only_count,
                        summary.unsupported_count,
                    ])?;
                }

                Ok(history_id)
            },
        )
        .map_err(AppError::Database)?;

    Ok(id)
}

/// Record a failed cleaning operation.
pub fn record_failed_cleaning(
    conn: &Connection,
    operation_id: &str,
    file_path: &str,
    error_message: &str,
) -> AppResult<()> {
    let timestamp = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO cleaning_history 
         (operation_id, file_path, backup_path, timestamp, operation_type, status, error_message)
         VALUES (?1, ?2, ?3, ?4, 'clean', 'failed', ?5)",
        params![operation_id, file_path, "", timestamp, error_message],
    )
    .map_err(AppError::Database)?;

    Ok(())
}

/// Record a restore operation.
pub fn record_restore(
    conn: &Connection,
    file_path: &str,
    backup_path: &str,
    error_message: Option<&str>,
) -> AppResult<()> {
    let timestamp = Utc::now().to_rfc3339();
    let operation_id = uuid::Uuid::new_v4().to_string();
    let status = if error_message.is_some() {
        "failed"
    } else {
        "success"
    };

    conn.execute(
        "INSERT INTO cleaning_history 
         (operation_id, file_path, backup_path, timestamp, operation_type, status, error_message)
         VALUES (?1, ?2, ?3, ?4, 'restore', ?5, ?6)",
        params![
            operation_id,
            file_path,
            backup_path,
            timestamp,
            status,
            error_message.unwrap_or(""),
        ],
    )
    .map_err(AppError::Database)?;

    Ok(())
}

// ──────────────────────────── Querying History ───────────────────────────────

/// Get all cleaning history entries, newest first.
pub fn get_history(conn: &Connection, limit: usize) -> AppResult<Vec<HistoryEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, operation_id, file_path, backup_path, timestamp, 
                    hash_before, hash_after, operation_type, status, error_message
             FROM cleaning_history 
             ORDER BY id DESC 
             LIMIT ?1",
        )
        .map_err(AppError::Database)?;

    let entries = stmt
        .query_map(params![limit], |row| {
            let timestamp_str: String = row.get(4)?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let operation_type_str: String = row.get(7)?;
            let operation_type = match operation_type_str.as_str() {
                "clean" => OperationType::Clean,
                "restore" => OperationType::Restore,
                _ => OperationType::Clean,
            };

            let status_str: String = row.get(8)?;
            let status = match status_str.as_str() {
                "success" => HistoryStatus::Success,
                "failed" => HistoryStatus::Failed,
                "rolled_back" => HistoryStatus::RolledBack,
                _ => HistoryStatus::Success,
            };

            // Load metadata summaries for this entry
            let history_id: i64 = row.get(0)?;
            let metadata_summary = get_metadata_summary_for_entry(conn, history_id)?;

            Ok(HistoryEntry {
                id: history_id,
                operation_id: row.get(1)?,
                file_path: row.get(2)?,
                backup_path: row.get(3)?,
                timestamp,
                hash_before: row.get(5)?,
                hash_after: row.get(6).ok(),
                operation_type,
                status,
                error_message: row.get(9).ok(),
                metadata_summary,
            })
        })
        .map_err(AppError::Database)?;

    entries.collect::<Result<Vec<_>, _>>().map_err(|e| {
        AppError::Database(rusqlite::Error::FromSqlConversion(
            0,
            "HistoryEntry".to_string(),
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        ))
    })
}

fn get_metadata_summary_for_entry(
    conn: &Connection,
    history_id: i64,
) -> AppResult<Vec<CategorySummary>> {
    let mut stmt = conn
        .prepare(
            "SELECT category, item_count, removable_count, partial_count, read_only_count, unsupported_count
             FROM metadata_summary
             WHERE cleaning_history_id = ?1",
        )
        .map_err(AppError::Database)?;

    let summaries = stmt
        .query_map(params![history_id], |row| {
            let cat_str: String = row.get(0)?;
            let category = serde_json::from_str(&cat_str).unwrap_or(MetadataCategory::Unknown);

            Ok(CategorySummary {
                category,
                item_count: row.get(1)?,
                removable_count: row.get(2)?,
                partial_count: row.get(3)?,
                read_only_count: row.get(4)?,
                unsupported_count: row.get(5)?,
            })
        })
        .map_err(AppError::Database)?;

    summaries.collect::<Result<Vec<_>, _>>().map_err(|e| {
        AppError::Unknown(format!("Failed to deserialize metadata summary: {}", e))
    })
}

/// Get a specific backup path for a file to enable restore.
pub fn get_latest_backup(conn: &Connection, file_path: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT backup_path FROM cleaning_history 
         WHERE file_path = ?1 AND backup_path != '' 
         ORDER BY id DESC LIMIT 1",
        params![file_path],
        |row| row.get(0),
    )
    .optional()
    .map_err(AppError::Database)
}

// ──────────────────────────── Cleanup ────────────────────────────────────────

/// Delete history entries older than a given number of days.
pub fn prune_history(conn: &Connection, days_old: i64) -> AppResult<usize> {
    let deleted = conn
        .execute(
            "DELETE FROM metadata_summary 
             WHERE cleaning_history_id IN (
                 SELECT id FROM cleaning_history 
                 WHERE datetime(timestamp) < datetime('now', ?1)
             );
             
             DELETE FROM cleaning_history 
             WHERE datetime(timestamp) < datetime('now', ?1);",
            params![format!("-{} days", days_old)],
        )
        .map_err(AppError::Database)?;

    Ok(deleted)
}

// ──────────────────────────── Clear History ─────────────────────────────────

/// Clear all history entries from the database.
pub fn clear_history(conn: &Connection) -> AppResult<usize> {
    conn.execute("DELETE FROM metadata_summary", [])
        .map_err(AppError::Database)?;
    let deleted = conn
        .execute("DELETE FROM cleaning_history", [])
        .map_err(AppError::Database)?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_db_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        for migration in MIGRATIONS {
            conn.execute_batch(migration).unwrap();
        }
        // Tables should exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count >= 2);
    }
}
