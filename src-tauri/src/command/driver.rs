use tauri::State;
use reqwest::Client;
use chrono::Utc;

use crate::AppState;
use super::types::{DriverConfig, ExecResponse, LsEntry};
use crate::crypto::{EncryptionAlgorithm, CryptoUtils};
use sqlx::Row;
use sha2::{Sha256, Digest};

#[derive(Clone)]
pub struct WebshellDriver {
    client: Client,
}

impl WebshellDriver {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("http client"),
        }
    }
}

#[tauri::command]
pub async fn configure_webshell(state: State<'_, AppState>, session_id: String, config: DriverConfig) -> Result<(), String> {
    // Store secret in memory (do not persist plain text)
    let mut guard = state.secrets.lock().map_err(|_| "secrets poisoned")?;
    guard.insert(session_id, config.password);
    drop(guard);
    // In a real impl, persist endpoint/charset if needed
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ws_execute(
    state: State<'_, AppState>,
    session_id: String,
    endpoint: String,
    command: String,
) -> Result<ExecResponse, String> {
    // Get session configuration including encryption settings
    let (secret, encryption_alg): (String, String) = if let Ok(row) = sqlx::query("SELECT secret_plain, encryption FROM sap_sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&state.pool)
        .await
    {
        (
            row.get::<Option<String>, _>("secret_plain").unwrap_or_default(),
            row.get::<String, _>("encryption")
        )
    } else {
        return Err("Session not found".to_string());
    };
    
    let secret = if !secret.is_empty() {
        secret
    } else {
        let guard = state.secrets.lock().map_err(|_| "secrets poisoned")?;
        guard.get(&session_id).cloned().ok_or_else(|| "No secret configured for session".to_string())?
    };

    // Parse encryption algorithm
    let algorithm = EncryptionAlgorithm::from_str(&encryption_alg)
        .map_err(|e| format!("Invalid encryption algorithm: {}", e))?;

    let resp = if algorithm == EncryptionAlgorithm::None {
        // No encryption - use original method
        Client::new()
            .post(&endpoint)
            .form(&[("pwd", secret.as_str()), ("cmd", command.as_str())])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
    } else {
        // Use encryption
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let key = hasher.finalize();
        
        let encrypted_request = CryptoUtils::encrypt_command(&command, &algorithm, &key)
            .await
            .map_err(|e| format!("Encryption failed: {}", e))?;
        
        println!("encrypted_request: {:?}", encrypted_request);
        Client::new()
            .post(&endpoint)
            .form(&[
                ("encrypted_data", encrypted_request.encrypted_data.as_str()),
                ("algorithm", encrypted_request.algorithm.as_str()),
                ("nonce", encrypted_request.nonce.as_deref().unwrap_or(""))
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
    };

    let response_text = resp.text().await.map_err(|e| format!("Read body failed: {}", e))?;
    
    let stdout = if algorithm != EncryptionAlgorithm::None {
        // Decrypt response
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let key = hasher.finalize();
        
        CryptoUtils::decrypt_response(&response_text, &algorithm, &key)
            .await
            .map_err(|e| format!("Decryption failed: {}", e))?
    } else {
        response_text
    };
    
    Ok(ExecResponse { stdout, stderr: String::new(), exit_code: 0, cwd: "/".into() })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn ws_list(
    state: State<'_, AppState>,
    session_id: String,
    endpoint: String,
    path: String,
) -> Result<Vec<LsEntry>, String> {
    let secret: String = if let Ok(row) = sqlx::query("SELECT secret_plain FROM sap_sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&state.pool)
        .await
    {
        row.get::<Option<String>, _>("secret_plain").unwrap_or_default()
    } else {
        String::new()
    };
    let secret = if !secret.is_empty() {
        secret
    } else {
        let guard = state.secrets.lock().map_err(|_| "secrets poisoned")?;
        guard.get(&session_id).cloned().ok_or_else(|| "No secret configured for session".to_string())?
    };

    // For demo: request remote to list raw and parse naive lines: "type name size perm mtime"
    let path_clone = path.clone();
    let list_cmd = format!("ls -la --time-style='+%Y-%m-%d %H:%M:%S' {}", shell_escape::escape(path_clone.into()));
    let out = ws_execute(state, session_id, endpoint, list_cmd).await?;
    let mut entries = Vec::new();
    for line in out.stdout.lines() {
        if line.starts_with("total ") || line.is_empty() { continue; }
        // very naive parser fallback
        let name = line.split_whitespace().last().unwrap_or("").to_string();
        let is_dir = line.starts_with('d');
        entries.push(LsEntry {
            name: name.clone(),
            path: if path.ends_with('/') { format!("{}{}", path, name) } else { format!("{}/{}", path, name) },
            r#type: if is_dir { "directory" } else { "file" }.into(),
            size: 0,
            perm: line.chars().take(10).collect(),
            mtime: Utc::now().to_rfc3339(),
            hidden: name.starts_with('.'),
        });
    }
    Ok(entries)
}

