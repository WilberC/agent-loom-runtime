use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

pub struct State {
    connection: Mutex<Connection>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub runtime_id: String,
    pub runtime_secret: String,
}
#[derive(Debug, Clone)]
pub struct PendingEvent {
    pub id: i64,
    pub kind: String,
    pub payload: String,
}
#[derive(Debug, Clone)]
pub struct Status {
    pub identity_present: bool,
    pub queued_events: i64,
    pub known_commands: i64,
    pub last_error: Option<String>,
}
impl State {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)
            .with_context(|| format!("open state database {}", path.display()))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS identity (singleton INTEGER PRIMARY KEY CHECK(singleton=1), runtime_id TEXT NOT NULL, runtime_secret TEXT NOT NULL); CREATE TABLE IF NOT EXISTS commands (command_id TEXT PRIMARY KEY, payload TEXT NOT NULL, received_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS outgoing_events (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, payload TEXT NOT NULL, created_at TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT); CREATE TABLE IF NOT EXISTS runtime_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
    pub fn identity(&self) -> Result<Option<Identity>> {
        let c = self.connection.lock().expect("state mutex poisoned");
        c.query_row(
            "SELECT runtime_id, runtime_secret FROM identity WHERE singleton=1",
            [],
            |r| {
                Ok(Identity {
                    runtime_id: r.get(0)?,
                    runtime_secret: r.get(1)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }
    pub fn save_identity(&self, id: &Uuid, secret: &str) -> Result<()> {
        self.connection.lock().expect("state mutex poisoned").execute("INSERT OR REPLACE INTO identity(singleton,runtime_id,runtime_secret) VALUES(1,?1,?2)", params![id.to_string(),secret])?;
        Ok(())
    }
    pub fn record_command(&self, command_id: Uuid, payload: &Value) -> Result<bool> {
        let n = self
            .connection
            .lock()
            .expect("state mutex poisoned")
            .execute(
                "INSERT OR IGNORE INTO commands(command_id,payload,received_at) VALUES(?1,?2,?3)",
                params![
                    command_id.to_string(),
                    payload.to_string(),
                    Utc::now().to_rfc3339()
                ],
            )?;
        Ok(n == 1)
    }
    pub fn queue_event(&self, kind: &str, payload: &Value) -> Result<()> {
        self.connection
            .lock()
            .expect("state mutex poisoned")
            .execute(
                "INSERT INTO outgoing_events(kind,payload,created_at) VALUES(?1,?2,?3)",
                params![kind, payload.to_string(), Utc::now().to_rfc3339()],
            )?;
        Ok(())
    }
    pub fn pending_events(&self) -> Result<Vec<PendingEvent>> {
        let c = self.connection.lock().expect("state mutex poisoned");
        let mut statement =
            c.prepare("SELECT id,kind,payload FROM outgoing_events ORDER BY id LIMIT 100")?;
        statement
            .query_map([], |r| {
                Ok(PendingEvent {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    payload: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
    pub fn delivered(&self, id: i64) -> Result<()> {
        self.connection
            .lock()
            .expect("state mutex poisoned")
            .execute("DELETE FROM outgoing_events WHERE id=?1", params![id])?;
        Ok(())
    }
    pub fn failed_delivery(&self, id: i64, error: &str) -> Result<()> {
        self.connection
            .lock()
            .expect("state mutex poisoned")
            .execute(
                "UPDATE outgoing_events SET attempts=attempts+1,last_error=?2 WHERE id=?1",
                params![id, error],
            )?;
        self.set_meta("last_error", error)
    }
    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.connection
            .lock()
            .expect("state mutex poisoned")
            .execute(
                "INSERT OR REPLACE INTO runtime_meta(key,value) VALUES(?1,?2)",
                params![key, value],
            )?;
        Ok(())
    }
    pub fn status(&self) -> Result<Status> {
        let c = self.connection.lock().expect("state mutex poisoned");
        Ok(Status {
            identity_present: c.query_row("SELECT EXISTS(SELECT 1 FROM identity)", [], |r| {
                r.get::<_, i64>(0)
            })? != 0,
            queued_events: c.query_row("SELECT count(*) FROM outgoing_events", [], |r| r.get(0))?,
            known_commands: c.query_row("SELECT count(*) FROM commands", [], |r| r.get(0))?,
            last_error: c
                .query_row(
                    "SELECT value FROM runtime_meta WHERE key='last_error'",
                    [],
                    |r| r.get(0),
                )
                .optional()?,
        })
    }
}
