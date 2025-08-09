use sqlx::{SqlitePool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde_json;

use crate::error::{AuroraResult, AuroraError};
use super::types::{Session, ProxyConfig, HeartbeatConfig};

pub struct SessionPersistence {
    pool: SqlitePool,
}

impl SessionPersistence {
    pub async fn new(database_url: &str) -> AuroraResult<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        
        // Create sessions table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                operator_id TEXT NOT NULL,
                target TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_activity TEXT NOT NULL,
                status TEXT NOT NULL,
                proxy_config TEXT,
                heartbeat_config TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Create session_logs table for audit trail
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                event_data TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions (id)
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn save_session(&self, session: &Session) -> AuroraResult<()> {
        let proxy_config_json = match &session.proxy_config {
            Some(config) => Some(serde_json::to_string(config)?),
            None => None,
        };

        let heartbeat_config_json = serde_json::to_string(&session.heartbeat_config)?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sessions 
            (id, operator_id, target, created_at, last_activity, status, proxy_config, heartbeat_config)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session.id.to_string())
        .bind(&session.operator_id)
        .bind(&session.target)
        .bind(session.created_at.to_rfc3339())
        .bind(session.last_activity.to_rfc3339())
        .bind(serde_json::to_string(&session.status)?)
        .bind(proxy_config_json)
        .bind(heartbeat_config_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_session(&self, session_id: &Uuid) -> AuroraResult<Option<Session>> {
        let row = sqlx::query(
            "SELECT * FROM sessions WHERE id = ?"
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let proxy_config: Option<ProxyConfig> = match row.get::<Option<String>, _>("proxy_config") {
                    Some(json) => Some(serde_json::from_str(&json)?),
                    None => None,
                };

                let heartbeat_config: HeartbeatConfig = serde_json::from_str(
                    &row.get::<String, _>("heartbeat_config")
                )?;

                let session = Session {
                    id: Uuid::parse_str(&row.get::<String, _>("id"))
                        .map_err(|e| AuroraError::Generic(e.into()))?,
                    operator_id: row.get("operator_id"),
                    target: row.get("target"),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                        .map_err(|e| AuroraError::Generic(e.into()))?
                        .with_timezone(&Utc),
                    last_activity: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_activity"))
                        .map_err(|e| AuroraError::Generic(e.into()))?
                        .with_timezone(&Utc),
                    status: serde_json::from_str(&row.get::<String, _>("status"))?,
                    proxy_config,
                    heartbeat_config,
                };

                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    pub async fn load_all_sessions(&self) -> AuroraResult<Vec<Session>> {
        let rows = sqlx::query("SELECT * FROM sessions")
            .fetch_all(&self.pool)
            .await?;

        let mut sessions = Vec::new();
        for row in rows {
            let proxy_config: Option<ProxyConfig> = match row.get::<Option<String>, _>("proxy_config") {
                Some(json) => Some(serde_json::from_str(&json)?),
                None => None,
            };

            let heartbeat_config: HeartbeatConfig = serde_json::from_str(
                &row.get::<String, _>("heartbeat_config")
            )?;

            let session = Session {
                id: Uuid::parse_str(&row.get::<String, _>("id"))
                    .map_err(|e| AuroraError::Generic(e.into()))?,
                operator_id: row.get("operator_id"),
                target: row.get("target"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
                last_activity: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_activity"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
                proxy_config,
                heartbeat_config,
            };

            sessions.push(session);
        }

        Ok(sessions)
    }

    pub async fn delete_session(&self, session_id: &Uuid) -> AuroraResult<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn log_session_event(
        &self,
        session_id: &Uuid,
        event_type: &str,
        event_data: Option<&str>,
    ) -> AuroraResult<()> {
        sqlx::query(
            r#"
            INSERT INTO session_logs (session_id, event_type, event_data, timestamp)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(session_id.to_string())
        .bind(event_type)
        .bind(event_data)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_session_logs(&self, session_id: &Uuid) -> AuroraResult<Vec<SessionLogEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM session_logs WHERE session_id = ? ORDER BY timestamp DESC"
        )
        .bind(session_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut logs = Vec::new();
        for row in rows {
            let log_entry = SessionLogEntry {
                id: row.get("id"),
                session_id: Uuid::parse_str(&row.get::<String, _>("session_id"))
                    .map_err(|e| AuroraError::Generic(e.into()))?,
                event_type: row.get("event_type"),
                event_data: row.get("event_data"),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
            };
            logs.push(log_entry);
        }

        Ok(logs)
    }

    pub async fn get_active_sessions(&self) -> AuroraResult<Vec<Session>> {
        let rows = sqlx::query("SELECT * FROM sessions WHERE status = ?")
            .bind(serde_json::to_string(&super::types::SessionStatus::Active)?)
            .fetch_all(&self.pool)
            .await?;

        let mut sessions = Vec::new();
        for row in rows {
            let proxy_config: Option<ProxyConfig> = match row.get::<Option<String>, _>("proxy_config") {
                Some(json) => Some(serde_json::from_str(&json)?),
                None => None,
            };

            let heartbeat_config: HeartbeatConfig = serde_json::from_str(
                &row.get::<String, _>("heartbeat_config")
            )?;

            let session = Session {
                id: Uuid::parse_str(&row.get::<String, _>("id"))
                    .map_err(|e| AuroraError::Generic(e.into()))?,
                operator_id: row.get("operator_id"),
                target: row.get("target"),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
                last_activity: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_activity"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
                proxy_config,
                heartbeat_config,
            };

            sessions.push(session);
        }

        Ok(sessions)
    }
}

#[derive(Debug, Clone)]
pub struct SessionLogEntry {
    pub id: i64,
    pub session_id: Uuid,
    pub event_type: String,
    pub event_data: Option<String>,
    pub timestamp: DateTime<Utc>,
}