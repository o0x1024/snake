use serde::{Deserialize, Serialize};
use tauri::State;
use sqlx::{SqlitePool, Row};
use chrono::Utc;
use uuid::Uuid;
use rand::{RngCore, rngs::OsRng};
use ring::digest;
use base64;

use crate::{AppState, SessionCreateConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSession {
    pub id: String,
    pub target: String,
    pub status: String,
    pub last_contact: String,
    pub encryption: String,
    pub uptime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSessionConfig {
    pub target: String,
    pub encryption: String,
    pub proxy: String,
    pub secret: Option<String>,
}

pub async fn init_db(pool: &SqlitePool) -> Result<(), String> {
    // Create sessions table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sap_sessions (
            id TEXT PRIMARY KEY,
            target TEXT NOT NULL,
            status TEXT NOT NULL,
            last_contact TEXT NOT NULL,
            encryption TEXT NOT NULL,
            uptime INTEGER NOT NULL,
            secret_provided INTEGER NOT NULL DEFAULT 0,
            secret_hash TEXT,
            secret_salt TEXT,
            secret_plain TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to init sessions table: {}", e))?;

    // Create command history table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS command_history (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            command TEXT NOT NULL,
            output TEXT,
            exit_code INTEGER NOT NULL DEFAULT 0,
            directory TEXT,
            timestamp TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'success',
            FOREIGN KEY (session_id) REFERENCES sap_sessions (id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to init command_history table: {}", e))?;

    // Create index for faster queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_command_history_session_id ON command_history (session_id)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_command_history_timestamp ON command_history (timestamp)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

    // Ensure columns exist for older DBs
    let rows = sqlx::query("SELECT name FROM pragma_table_info('sap_sessions')")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to read table info: {}", e))?;

    let mut has_provided = false;
    let mut has_hash = false;
    let mut has_salt = false;
    let mut has_plain = false;
    for row in rows {
        let name: String = row.get("name");
        if name == "secret_provided" { has_provided = true; }
        if name == "secret_hash" { has_hash = true; }
        if name == "secret_salt" { has_salt = true; }
        if name == "secret_plain" { has_plain = true; }
    }
    if !has_provided {
        sqlx::query("ALTER TABLE sap_sessions ADD COLUMN secret_provided INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to add column secret_provided: {}", e))?;
    }
    if !has_hash {
        sqlx::query("ALTER TABLE sap_sessions ADD COLUMN secret_hash TEXT")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to add column secret_hash: {}", e))?;
    }
    if !has_salt {
        sqlx::query("ALTER TABLE sap_sessions ADD COLUMN secret_salt TEXT")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to add column secret_salt: {}", e))?;
    }
    if !has_plain {
        sqlx::query("ALTER TABLE sap_sessions ADD COLUMN secret_plain TEXT")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to add column secret_plain: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_active_sessions(state: State<'_, AppState>, _token: String) -> Result<Vec<FrontendSession>, String> {
    let rows = sqlx::query("SELECT id, target, status, last_contact, encryption, uptime FROM sap_sessions WHERE status = 'active'")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| format!("Failed to query sessions: {}", e))?;

    let mut list = Vec::new();
    for row in rows {
        list.push(FrontendSession {
            id: row.get::<String, _>("id"),
            target: row.get::<String, _>("target"),
            status: row.get::<String, _>("status"),
            last_contact: row.get::<String, _>("last_contact"),
            encryption: row.get::<String, _>("encryption"),
            uptime: row.get::<i64, _>("uptime") as u64,
        });
    }
    Ok(list)
}

#[tauri::command]
pub async fn create_session(state: State<'_, AppState>, _token: String, config: FrontendSessionConfig) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let (secret_provided, secret_hash_b64, secret_salt_b64) = if let Some(secret) = config.secret.as_ref() {
        if !secret.is_empty() {
            let mut salt = [0u8; 16];
            OsRng.fill_bytes(&mut salt);
            let mut data = Vec::with_capacity(salt.len() + secret.as_bytes().len());
            data.extend_from_slice(&salt);
            data.extend_from_slice(secret.as_bytes());
            let digest = digest::digest(&digest::SHA256, &data);
            (1i64, Some(base64::encode(digest.as_ref())), Some(base64::encode(salt)))
        } else {
            (0i64, None, None)
        }
    } else {
        (0i64, None, None)
    };

    sqlx::query(
        r#"INSERT INTO sap_sessions (id, target, status, last_contact, encryption, uptime, secret_provided, secret_hash, secret_salt, secret_plain)
           VALUES (?, ?, 'active', ?, ?, 0, ?, ?, ?, ?)"#,
    )
    .bind(&session_id)
    .bind(&config.target)
    .bind(&now)
    .bind(&config.encryption)
    .bind(secret_provided)
    .bind(secret_hash_b64)
    .bind(secret_salt_b64)
    .bind(config.secret.clone().unwrap_or_default())
    .execute(&state.pool)
    .await
    .map_err(|e| format!("Failed to create session: {}", e))?;

    Ok(session_id)
}

#[tauri::command]
pub async fn update_session_secret(
    state: State<'_, AppState>,
    _token: String,
    session_id: String,
    secret: String,
) -> Result<(), String> {
    if secret.trim().is_empty() {
        return Err("Secret cannot be empty".into());
    }

    // Recompute hash/salt for audit and store plain secret
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let mut data = Vec::with_capacity(salt.len() + secret.as_bytes().len());
    data.extend_from_slice(&salt);
    data.extend_from_slice(secret.as_bytes());
    let digest = digest::digest(&digest::SHA256, &data);
    let secret_hash_b64 = Some(base64::encode(digest.as_ref()));
    let secret_salt_b64 = Some(base64::encode(salt));

    sqlx::query(
        r#"UPDATE sap_sessions SET secret_provided = 1, secret_hash = ?, secret_salt = ?, secret_plain = ? WHERE id = ?"#,
    )
    .bind(secret_hash_b64)
    .bind(secret_salt_b64)
    .bind(secret)
    .bind(&session_id)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("Failed to update session secret: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn terminate_session(state: State<'_, AppState>, _token: String, session_id: String) -> Result<(), String> {
    sqlx::query("UPDATE sap_sessions SET status = 'terminated', last_contact = ? WHERE id = ?")
        .bind(Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .bind(&session_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to terminate session: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_session(state: State<'_, AppState>, _token: String, session_id: String) -> Result<(), String> {
    sqlx::query("DELETE FROM sap_sessions WHERE id = ?")
        .bind(&session_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to delete session: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn update_session(
    state: State<'_, AppState>,
    _token: String,
    session_id: String,
    target: Option<String>,
    encryption: Option<String>,
) -> Result<(), String> {
    if target.is_none() && encryption.is_none() {
        return Ok(());
    }
    let mut query = String::from("UPDATE sap_sessions SET ");
    let mut first = true;
    if target.is_some() {
        query.push_str("target = ?");
        first = false;
    }
    if encryption.is_some() {
        if !first { query.push_str(", "); }
        query.push_str("encryption = ?");
    }
    query.push_str(" WHERE id = ?");

    let mut q = sqlx::query(&query);
    if let Some(t) = target.as_ref() { q = q.bind(t); }
    if let Some(e) = encryption.as_ref() { q = q.bind(e); }
    q = q.bind(&session_id);

    q.execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to update session: {}", e))?;
    Ok(())
}

// Command History related structures and functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistoryEntry {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub output: String,
    pub exit_code: i32,
    pub directory: String,
    pub timestamp: String,
    pub status: String,
}

#[tauri::command]
pub async fn save_command_history(
    state: State<'_, AppState>,
    session_id: String,
    command_id: String,
    command: String,
    output: String,
    exit_code: i32,
    directory: String,
    status: String,
) -> Result<(), String> {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    
    sqlx::query(
        r#"INSERT INTO command_history (id, session_id, command, output, exit_code, directory, timestamp, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&command_id)
    .bind(&session_id)
    .bind(&command)
    .bind(&output)
    .bind(exit_code)
    .bind(&directory)
    .bind(&timestamp)
    .bind(&status)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("Failed to save command history: {}", e))?;
    
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_command_history(
    state: State<'_, AppState>,
    session_id: String,
    limit: Option<i32>,
) -> Result<Vec<CommandHistoryEntry>, String> {
    let limit_clause = if let Some(l) = limit {
        format!(" LIMIT {}", l)
    } else {
        String::new()
    };
    
    let query = format!(
        "SELECT id, session_id, command, output, exit_code, directory, timestamp, status FROM command_history WHERE session_id = ? ORDER BY timestamp DESC{}",
        limit_clause
    );
    
    let rows = sqlx::query(&query)
        .bind(&session_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| format!("Failed to query command history: {}", e))?;

    let mut history = Vec::new();
    for row in rows {
        history.push(CommandHistoryEntry {
            id: row.get::<String, _>("id"),
            session_id: row.get::<String, _>("session_id"),
            command: row.get::<String, _>("command"),
            output: row.get::<String, _>("output"),
            exit_code: row.get::<i32, _>("exit_code"),
            directory: row.get::<String, _>("directory"),
            timestamp: row.get::<String, _>("timestamp"),
            status: row.get::<String, _>("status"),
        });
    }
    
    Ok(history)
}

#[tauri::command]
pub async fn clear_command_history(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    sqlx::query("DELETE FROM command_history WHERE session_id = ?")
        .bind(&session_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to clear command history: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn update_session_heartbeat(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    
    sqlx::query("UPDATE sap_sessions SET last_contact = ? WHERE id = ?")
        .bind(&now)
        .bind(&session_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("Failed to update session heartbeat: {}", e))?;
    
    Ok(())
}
