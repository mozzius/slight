use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("store error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_id: String,
    pub metadata: Value,
    pub last_sequence: u64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredEvent {
    pub sequence: u64,
    pub event: Value,
}

pub trait SessionStore: Send {
    fn upsert_session(&mut self, session: StoredSession) -> StoreResult<()>;
    fn get_session(&self, session_id: &str) -> StoreResult<Option<StoredSession>>;
    fn list_sessions(&self) -> StoreResult<Vec<StoredSession>>;
    fn append_event(&mut self, session_id: &str, event: StoredEvent) -> StoreResult<()>;
    fn events_after(
        &self,
        session_id: &str,
        after: Option<u64>,
        limit: usize,
    ) -> StoreResult<Vec<StoredEvent>>;
    fn latest_sequence(&self, session_id: &str) -> StoreResult<u64>;
}

/// SQLite-backed session store. Schema creation is deliberately performed on
/// open so the same path works for a fresh host and a restarted host.
pub struct SqliteStore {
    connection: Connection,
    event_limit: usize,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<std::path::Path>, event_limit: usize) -> StoreResult<Self> {
        let connection = Connection::open(path).map_err(sqlite_error)?;
        Self::from_connection(connection, event_limit)
    }

    pub fn in_memory(event_limit: usize) -> StoreResult<Self> {
        Self::from_connection(
            Connection::open_in_memory().map_err(sqlite_error)?,
            event_limit,
        )
    }

    fn from_connection(connection: Connection, event_limit: usize) -> StoreResult<Self> {
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS schema_migrations (
                     version INTEGER PRIMARY KEY
                 );
                 CREATE TABLE IF NOT EXISTS sessions (
                     session_id TEXT PRIMARY KEY,
                     metadata TEXT NOT NULL,
                     last_sequence INTEGER NOT NULL,
                     created_at_ms INTEGER NOT NULL,
                     updated_at_ms INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS events (
                     session_id TEXT NOT NULL,
                     sequence INTEGER NOT NULL,
                     event TEXT NOT NULL,
                     PRIMARY KEY (session_id, sequence),
                     FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS events_session_sequence
                     ON events(session_id, sequence);",
            )
            .map_err(sqlite_error)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version) VALUES (1)",
                [],
            )
            .map_err(sqlite_error)?;
        Ok(Self {
            connection,
            event_limit,
        })
    }
}

impl SessionStore for SqliteStore {
    fn upsert_session(&mut self, session: StoredSession) -> StoreResult<()> {
        let metadata = serde_json::to_string(&session.metadata)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        self.connection
            .execute(
                "INSERT INTO sessions(session_id, metadata, last_sequence, created_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(session_id) DO UPDATE SET
                   metadata = excluded.metadata,
                   last_sequence = excluded.last_sequence,
                   created_at_ms = excluded.created_at_ms,
                   updated_at_ms = excluded.updated_at_ms",
                params![
                    session.session_id,
                    metadata,
                    session.last_sequence as i64,
                    session.created_at_ms,
                    session.updated_at_ms,
                ],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn get_session(&self, session_id: &str) -> StoreResult<Option<StoredSession>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT session_id, metadata, last_sequence, created_at_ms, updated_at_ms
                 FROM sessions WHERE session_id = ?1",
            )
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![session_id]).map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Ok(None);
        };
        Ok(Some(read_session(row).map_err(sqlite_error)?))
    }

    fn list_sessions(&self) -> StoreResult<Vec<StoredSession>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT session_id, metadata, last_sequence, created_at_ms, updated_at_ms
                 FROM sessions ORDER BY created_at_ms",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([], read_session)
            .map_err(sqlite_error)?;
        rows.map(|row| row.map_err(sqlite_error)).collect()
    }

    fn append_event(&mut self, session_id: &str, event: StoredEvent) -> StoreResult<()> {
        let value = serde_json::to_string(&event.event)
            .map_err(|error| StoreError::Serialization(error.to_string()))?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO events(session_id, sequence, event) VALUES (?1, ?2, ?3)",
                params![session_id, event.sequence as i64, value],
            )
            .map_err(sqlite_error)?;
        if self.event_limit > 0 {
            transaction
                .execute(
                    "DELETE FROM events WHERE session_id = ?1 AND sequence <= (
                         SELECT COALESCE(MAX(sequence), 0) - ?2 FROM events WHERE session_id = ?1
                     )",
                    params![session_id, self.event_limit as i64],
                )
                .map_err(sqlite_error)?;
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(())
    }

    fn events_after(
        &self,
        session_id: &str,
        after: Option<u64>,
        limit: usize,
    ) -> StoreResult<Vec<StoredEvent>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT sequence, event FROM events
                 WHERE session_id = ?1 AND sequence > ?2 ORDER BY sequence LIMIT ?3",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map(
                params![session_id, after.unwrap_or(0) as i64, limit as i64],
                |row| {
                    let event: String = row.get(1)?;
                    Ok(StoredEvent {
                        sequence: row.get::<_, i64>(0)? as u64,
                        event: serde_json::from_str(&event).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                    })
                },
            )
            .map_err(sqlite_error)?;
        rows.map(|row| row.map_err(sqlite_error)).collect()
    }

    fn latest_sequence(&self, session_id: &str) -> StoreResult<u64> {
        self.connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|sequence| sequence as u64)
            .map_err(sqlite_error)
    }
}

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSession> {
    let metadata: String = row.get(1)?;
    Ok(StoredSession {
        session_id: row.get(0)?,
        metadata: serde_json::from_str(&metadata).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        last_sequence: row.get::<_, i64>(2)? as u64,
        created_at_ms: row.get(3)?,
        updated_at_ms: row.get(4)?,
    })
}

fn sqlite_error(error: rusqlite::Error) -> StoreError {
    StoreError::Other(error.to_string())
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    sessions: HashMap<String, StoredSession>,
    events: HashMap<String, Vec<StoredEvent>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for InMemoryStore {
    fn upsert_session(&mut self, session: StoredSession) -> StoreResult<()> {
        self.sessions.insert(session.session_id.clone(), session);
        Ok(())
    }

    fn get_session(&self, session_id: &str) -> StoreResult<Option<StoredSession>> {
        Ok(self.sessions.get(session_id).cloned())
    }

    fn list_sessions(&self) -> StoreResult<Vec<StoredSession>> {
        let mut sessions: Vec<StoredSession> = self.sessions.values().cloned().collect();
        sessions.sort_by_key(|session| session.created_at_ms);
        Ok(sessions)
    }

    fn append_event(&mut self, session_id: &str, event: StoredEvent) -> StoreResult<()> {
        self.events
            .entry(session_id.to_string())
            .or_default()
            .push(event);
        Ok(())
    }

    fn events_after(
        &self,
        session_id: &str,
        after: Option<u64>,
        limit: usize,
    ) -> StoreResult<Vec<StoredEvent>> {
        let Some(events) = self.events.get(session_id) else {
            return Ok(Vec::new());
        };
        let after = after.unwrap_or(0);
        Ok(events
            .iter()
            .filter(|event| event.sequence > after)
            .take(limit)
            .cloned()
            .collect())
    }

    fn latest_sequence(&self, session_id: &str) -> StoreResult<u64> {
        Ok(self
            .events
            .get(session_id)
            .and_then(|events| events.last())
            .map(|event| event.sequence)
            .unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str) -> StoredSession {
        StoredSession {
            session_id: id.to_string(),
            metadata: serde_json::json!({"status": "idle"}),
            last_sequence: 0,
            created_at_ms: 10,
            updated_at_ms: 10,
        }
    }

    #[test]
    fn sqlite_round_trips_sessions_and_events() {
        let mut store = SqliteStore::in_memory(2).unwrap();
        store.upsert_session(session("s-1")).unwrap();
        for sequence in 1..=3 {
            store
                .append_event(
                    "s-1",
                    StoredEvent {
                        sequence,
                        event: serde_json::json!({"sequence": sequence}),
                    },
                )
                .unwrap();
        }

        assert_eq!(store.get_session("s-1").unwrap(), Some(session("s-1")));
        assert_eq!(store.latest_sequence("s-1").unwrap(), 3);
        let events = store.events_after("s-1", None, 10).unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn sqlite_reopens_existing_database() {
        let path =
            std::env::temp_dir().join(format!("slight-store-{}.sqlite", uuid::Uuid::new_v4()));
        {
            let mut store = SqliteStore::open(&path, 16).unwrap();
            store.upsert_session(session("persisted")).unwrap();
        }
        let store = SqliteStore::open(&path, 16).unwrap();
        assert_eq!(store.list_sessions().unwrap()[0].session_id, "persisted");
        std::fs::remove_file(path).unwrap();
    }
}
