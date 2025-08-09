use serde::{Deserialize, Serialize};
use tauri::State;
use crate::AppState;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub output: String,
    pub exit_code: i32,
    pub directory: String,
}

#[tauri::command]
pub async fn execute_command(
    state: State<'_, AppState>,
    session_id: String,
    command: String,
    command_id: String,
) -> Result<CommandResult, String> {
    // Very limited, local demo executor. In production, connect to remote session.
    let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
    let arg = if cfg!(target_os = "windows") { "/C" } else { "-c" };

    let output = tokio::process::Command::new(shell)
        .arg(arg)
        .arg(&command)
        .output()
        .await
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/".into());

    let result = CommandResult {
        output: combined.clone(),
        exit_code: output.status.code().unwrap_or(-1),
        directory: cwd.clone(),
    };
    
    // Save command history to database
    let actual_command_id = if command_id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        command_id
    };
    
    let status = if result.exit_code == 0 { "success" } else { "error" };
    
    if let Err(e) = crate::command::session::save_command_history(
        state,
        session_id,
        actual_command_id,
        command,
        combined,
        result.exit_code,
        cwd,
        status.to_string(),
    ).await {
        tracing::warn!("Failed to save command history: {}", e);
    }
    
    Ok(result)
}

