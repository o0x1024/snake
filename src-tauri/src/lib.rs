//! Security Assessment Platform
//! 
//! A comprehensive desktop application for authorized security testing,
//! vulnerability assessment, and compliance auditing.

// Core modules
pub mod error;
pub mod traits;
pub mod session;
pub mod crypto;
pub mod fs;
pub mod net;
pub mod plugins;
pub mod command;

// Re-export core types and traits
pub use error::{AuroraError, AuroraResult};
pub use traits::{WebshellExecutor, StealthTransport, ComplianceValidator};

// Authentication and session types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub target: String,
    pub status: String,
    pub last_contact: String,
    pub encryption: String,
    pub uptime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreateConfig {
    pub target: String,
    pub encryption: String,
    pub proxy: String,
    pub secret: Option<String>,
}

// Deprecated in-memory store removed in favor of DB-backed state

use tracing::info;
use serde::{Deserialize, Serialize};
 
 
use sqlx::{SqlitePool};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tauri::Manager;
use std::path::PathBuf;
use std::fs as stdfs;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use session::SessionManager;

pub struct AppState {
    pub pool: SqlitePool,
    // In-memory ephemeral secrets keyed by session id
    pub secrets: Arc<Mutex<HashMap<String, String>>>,
    // Session manager for heartbeat and advanced session management
    pub session_manager: Arc<SessionManager>,
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Security Assessment Platform");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tauri::async_runtime::block_on(async {
                // Resolve writable app data directory using Tauri path resolver
                let db_dir: PathBuf = match app.path().app_data_dir() {
                    Ok(p) => p,
                    Err(e) => {
                        return Err(format!("Failed to resolve app_data_dir: {}", e));
                    }
                };
                stdfs::create_dir_all(&db_dir)
                    .map_err(|e| format!("Failed to create DB dir: {}", e))?;
                let db_path = db_dir.join("sap.db");
                let db_uri = format!("sqlite://{}", db_path.to_string_lossy());

                // Use connect options with create_if_missing for reliability
                let options = SqliteConnectOptions::from_str(&db_uri)
                    .map_err(|e| format!("Invalid DB URI: {}", e))?
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .connect_with(options)
                    .await
                    .map_err(|e| format!("Failed to connect DB: {}", e))?;
                if let Err(e) = command::session::init_db(&pool).await {
                    return Err(format!("DB init failed: {}", e));
                }
                
                // Initialize session manager with heartbeat enabled
                let session_config = session::SessionConfig {
                    timeout_minutes: 30,
                    max_concurrent_sessions: 10,
                    enable_heartbeat: true,
                    heartbeat_interval_seconds: 10,
                };
                
                let session_manager = SessionManager::new(session_config)
                    .with_persistence(&db_uri)
                    .await
                    .map_err(|e| format!("Failed to initialize session manager: {}", e))?;
                
                // Start heartbeat manager
                if let Err(e) = session_manager.start_heartbeat_manager().await {
                    tracing::warn!("Failed to start heartbeat manager: {}", e);
                }
                
                let session_manager = Arc::new(session_manager);
                
                app.manage(AppState { 
                    pool, 
                    secrets: Arc::new(Mutex::new(HashMap::new())),
                    session_manager,
                });
                Ok::<(), String>(())
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // DB-backed session commands
            command::session::get_active_sessions,
            command::session::create_session,
            command::session::terminate_session,
            command::session::delete_session,
            command::session::update_session,
            command::session::update_session_secret,
            // File system commands
            command::fs::list_directory,
            command::fs::download_file,
            command::fs::download_file_with_endpoint,
            command::fs::delete_files,
            command::fs::upload_file,
            command::fs::read_file,
            command::fs::write_file,
            command::fs::rename_file,
            command::fs::create_directory,
            command::fs::copy_file,
            command::fs::get_file_info,
            // Exec commands
            command::exec::execute_command,
            // Command history commands
            command::session::save_command_history,
            command::session::get_command_history,
            command::session::clear_command_history,
            command::session::update_session_heartbeat,
            // Webshell driver commands
            command::driver::configure_webshell,
            command::driver::ws_execute,
            command::driver::ws_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
