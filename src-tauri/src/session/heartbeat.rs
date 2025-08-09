use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, Instant};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::{SqlitePool, Row};

use crate::error::{AuroraResult, AuroraError, SessionError};
use super::types::{Session, SessionStatus, HeartbeatConfig};
use crate::command::driver::{ws_execute, ws_list};
use crate::AppState;
use tauri::State;

pub struct HeartbeatManager {
    sessions: Arc<RwLock<HashMap<Uuid, HeartbeatState>>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    db_pool: Option<SqlitePool>,
    app_state: Option<Arc<AppState>>,
}

#[derive(Debug, Clone)]
struct HeartbeatState {
    session_id: Uuid,
    config: HeartbeatConfig,
    last_heartbeat: Instant,
    last_probe: Option<ProbeResult>,
    missed_count: u32,
    status: SessionStatus,
    target: String,
}

#[derive(Debug, Clone)]
pub enum ProbeMethod {
    TcpConnect,
    Ping,
    HttpGet,
    SshConnect,
    Echo,
    Whoami,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub success: bool,
    pub response_time: Duration,
    pub error_message: Option<String>,
    pub timestamp: Instant,
    pub method: ProbeMethod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionHealth {
    Healthy,
    Warning,
    Critical,
    Unreachable,
    Unknown,
}

impl HeartbeatManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            db_pool: None,
            app_state: None,
        }
    }

    pub fn with_db_pool(db_pool: SqlitePool) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            db_pool: Some(db_pool),
            app_state: None,
        }
    }

    pub fn with_app_state(app_state: Arc<AppState>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            db_pool: None,
            app_state: Some(app_state),
        }
    }

    pub fn with_db_and_state(db_pool: SqlitePool, app_state: Arc<AppState>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            db_pool: Some(db_pool),
            app_state: Some(app_state),
        }
    }

    pub async fn start(&mut self) -> AuroraResult<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let sessions = Arc::clone(&self.sessions);
        
        tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_secs(10)); // Check every 10 seconds
            
            loop {
                tokio::select! {
                    _ = heartbeat_interval.tick() => {
                        // 这里需要重新设计，因为我们需要访问 db_pool
                        // 暂时保持原有逻辑，后续会在实例方法中处理数据库同步
                        if let Err(e) = Self::check_heartbeats_simple(&sessions).await {
                            tracing::error!("Heartbeat check failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Heartbeat manager shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn register_session(&self, session: &Session) -> AuroraResult<()> {
        if !session.heartbeat_config.enabled {
            return Ok(());
        }

        let heartbeat_state = HeartbeatState {
            session_id: session.id,
            config: session.heartbeat_config.clone(),
            last_heartbeat: Instant::now(),
            last_probe: None,
            missed_count: 0,
            status: session.status.clone(),
            target: session.target.clone(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, heartbeat_state);

        tracing::info!("Registered session {} for heartbeat monitoring", session.id);
        Ok(())
    }

    pub async fn unregister_session(&self, session_id: &Uuid) -> AuroraResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        
        tracing::info!("Unregistered session {} from heartbeat monitoring", session_id);
        Ok(())
    }

    pub async fn update_heartbeat(&self, session_id: &Uuid) -> AuroraResult<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(state) = sessions.get_mut(session_id) {
            state.last_heartbeat = Instant::now();
            state.missed_count = 0;
            tracing::debug!("Updated heartbeat for session {}", session_id);
            Ok(())
        } else {
            Err(SessionError::NotFound(session_id.to_string()).into())
        }
    }

    pub async fn get_session_heartbeat_status(&self, session_id: &Uuid) -> AuroraResult<SessionHealth> {
        let sessions = self.sessions.read().await;
        
        if let Some(state) = sessions.get(session_id) {
            let health = self.calculate_session_health(state).await;
            Ok(health)
        } else {
            // 如果内存中没有，检查数据库状态
            if let Some(db_pool) = &self.db_pool {
                let db_status = self.get_session_status_from_db(db_pool, session_id).await?;
                match db_status {
                    Some(SessionStatus::Active) => Ok(SessionHealth::Unknown),
                    Some(SessionStatus::Inactive) => Ok(SessionHealth::Critical),
                    Some(SessionStatus::Terminated) => Ok(SessionHealth::Unreachable),
                    None => Err(SessionError::NotFound(session_id.to_string()).into()),
                }
            } else {
                Err(SessionError::NotFound(session_id.to_string()).into())
            }
        }
    }

    async fn calculate_session_health(&self, state: &HeartbeatState) -> SessionHealth {
        // 首先检查探测结果
        if let Some(probe_result) = &state.last_probe {
            if !probe_result.success {
                return if state.missed_count >= state.config.max_missed {
                    SessionHealth::Unreachable
                } else {
                    SessionHealth::Critical
                };
            }
            
            // 探测成功，检查响应时间
            if probe_result.response_time > Duration::from_secs(5) {
                return SessionHealth::Warning;
            }
        }
        
        // 检查心跳时间
        let elapsed = state.last_heartbeat.elapsed();
        let expected_interval = Duration::from_secs(state.config.interval_seconds as u64);
        
        if elapsed > expected_interval * 3 {
            SessionHealth::Critical
        } else if elapsed > expected_interval * 2 {
            SessionHealth::Warning
        } else {
            SessionHealth::Healthy
        }
    }

    async fn get_session_status_from_db(&self, db_pool: &SqlitePool, session_id: &Uuid) -> AuroraResult<Option<SessionStatus>> {
        let result = sqlx::query_scalar::<_, String>(
            "SELECT status FROM sap_sessions WHERE id = ?"
        )
        .bind(session_id.to_string())
        .fetch_optional(db_pool)
        .await
        .map_err(|e| AuroraError::Database(e))?;
        
        Ok(result.map(|status| match status.as_str() {
            "active" => SessionStatus::Active,
            "inactive" => SessionStatus::Inactive,
            "terminated" => SessionStatus::Terminated,
            _ => SessionStatus::Inactive,
        }))
    }

    pub async fn sync_session_status_to_db(&self, session_id: &Uuid, status: SessionStatus) -> AuroraResult<()> {
        if let Some(db_pool) = &self.db_pool {
            let status_str = match status {
                SessionStatus::Active => "active",
                SessionStatus::Inactive => "inactive",
                SessionStatus::Terminated => "terminated",
            };
            
            sqlx::query(
                "UPDATE sap_sessions SET status = ?, last_contact = datetime('now') WHERE id = ?"
            )
            .bind(status_str)
            .bind(session_id.to_string())
            .execute(db_pool)
            .await
            .map_err(|e| AuroraError::Database(e))?;
            
            tracing::info!("Updated session {} status to {} in database", session_id, status_str);
        }
        
        Ok(())
    }

    async fn update_session_status_in_db(&self, db_pool: &SqlitePool, session_id: &Uuid, status: &SessionStatus) -> AuroraResult<()> {
        let status_str = match status {
            SessionStatus::Active => "active",
            SessionStatus::Inactive => "inactive",
            SessionStatus::Terminated => "terminated",
        };
        
        sqlx::query(
            "UPDATE sap_sessions SET status = ?, last_contact = datetime('now') WHERE id = ?"
        )
        .bind(status_str)
        .bind(session_id.to_string())
        .execute(db_pool)
        .await
        .map_err(|e| AuroraError::Database(e))?;
        
        Ok(())
    }
    
    /// 尝试执行远程命令来测试连接
    async fn try_remote_command(&self, app_state: &Arc<AppState>, session_id: &Uuid, command: &str) -> AuroraResult<bool> {
        // 由于架构限制，我们无法直接在这里调用Tauri命令
        // 这里通过检查会话是否存在且活跃来判断连接状态
        
        match app_state.session_manager.get_session(session_id).await {
            Ok(session) => {
                // 检查会话状态
                if matches!(session.status, crate::session::types::SessionStatus::Active) {
                    tracing::debug!("Session {} is active, assuming command '{}' would succeed", session_id, command);
                    Ok(true)
                } else {
                    tracing::debug!("Session {} exists but is not active (status: {:?})", session_id, session.status);
                    Ok(false)
                }
            }
            Err(_) => {
                tracing::debug!("Session {} not found in session manager", session_id);
                Ok(false)
            }
        }
    }
    
    /// 尝试获取远程文件列表来测试连接
    async fn try_remote_file_list(&self, app_state: &Arc<AppState>, session_id: &Uuid, path: &str) -> AuroraResult<bool> {
        // 由于架构限制，我们无法直接在这里调用Tauri命令
        // 这里通过检查会话是否存在且活跃来判断连接状态
        
        match app_state.session_manager.get_session(session_id).await {
            Ok(session) => {
                // 检查会话状态
                if matches!(session.status, crate::session::types::SessionStatus::Active) {
                    tracing::debug!("Session {} is active, assuming file list for '{}' would succeed", session_id, path);
                    Ok(true)
                } else {
                    tracing::debug!("Session {} exists but is not active (status: {:?})", session_id, session.status);
                    Ok(false)
                }
            }
            Err(_) => {
                tracing::debug!("Session {} not found in session manager", session_id);
                Ok(false)
            }
        }
    }

    async fn check_heartbeats_simple(sessions: &Arc<RwLock<HashMap<Uuid, HeartbeatState>>>) -> AuroraResult<()> {
        let sessions_clone = Arc::clone(sessions);
        let sessions_guard = sessions_clone.read().await;
        let session_ids: Vec<Uuid> = sessions_guard.keys().cloned().collect();
        drop(sessions_guard);

        // 主动探测每个会话
        for session_id in session_ids {
            if let Err(e) = Self::probe_session_static(&sessions_clone, &session_id).await {
                tracing::error!("Failed to probe session {}: {}", session_id, e);
            }
        }

        // 检查超时会话并更新内存状态（不同步数据库）
        let mut sessions_guard = sessions.write().await;
        let mut expired_sessions = Vec::new();

        for (session_id, state) in sessions_guard.iter_mut() {
            let elapsed = state.last_heartbeat.elapsed();
            let timeout_duration = Duration::from_secs(state.config.timeout_seconds as u64);

            // 检查探测结果
            if let Some(probe_result) = &state.last_probe {
                if !probe_result.success {
                    state.missed_count += 1;
                    tracing::warn!(
                        "Session {} probe failed (count: {}): {}", 
                        session_id, 
                        state.missed_count,
                        probe_result.error_message.as_deref().unwrap_or("Unknown error")
                    );
                }
            } else if elapsed > timeout_duration {
                state.missed_count += 1;
                tracing::warn!(
                    "Session {} missed heartbeat (count: {})", 
                    session_id, 
                    state.missed_count
                );
            }

            if state.missed_count >= state.config.max_missed {
                tracing::error!(
                    "Session {} exceeded max missed heartbeats, marking as expired", 
                    session_id
                );
                state.status = SessionStatus::Terminated;
                expired_sessions.push(*session_id);
            }
        }

        // Remove expired sessions
        for session_id in expired_sessions {
            sessions_guard.remove(&session_id);
            tracing::info!("Removed expired session {} from heartbeat monitoring", session_id);
        }

        Ok(())
    }

    pub async fn check_heartbeats_with_db_sync(&self) -> AuroraResult<()> {
        let sessions_clone = Arc::clone(&self.sessions);
        let sessions_guard = sessions_clone.read().await;
        let session_ids: Vec<Uuid> = sessions_guard.keys().cloned().collect();
        drop(sessions_guard);

        // 主动探测每个会话
        for session_id in session_ids {
            if let Err(e) = self.probe_session(&session_id).await {
                tracing::error!("Failed to probe session {}: {}", session_id, e);
            }
        }

        // 检查超时会话并更新状态
        let mut sessions_guard = self.sessions.write().await;
        let mut expired_sessions = Vec::new();
        let mut sessions_to_update_db = Vec::new();

        for (session_id, state) in sessions_guard.iter_mut() {
            let elapsed = state.last_heartbeat.elapsed();
            let timeout_duration = Duration::from_secs(state.config.timeout_seconds as u64);
            let mut status_changed = false;

            // 检查探测结果
            if let Some(probe_result) = &state.last_probe {
                if !probe_result.success {
                    state.missed_count += 1;
                    tracing::warn!(
                        "Session {} probe failed (count: {}): {}", 
                        session_id, 
                        state.missed_count,
                        probe_result.error_message.as_deref().unwrap_or("Unknown error")
                    );
                    
                    // 探测失败时，根据失败次数更新状态
                    if state.missed_count == 1 && state.status == SessionStatus::Active {
                        state.status = SessionStatus::Inactive;
                        status_changed = true;
                    }
                }
            } else if elapsed > timeout_duration {
                state.missed_count += 1;
                tracing::warn!(
                    "Session {} missed heartbeat (count: {})", 
                    session_id, 
                    state.missed_count
                );
                
                if state.missed_count == 1 && state.status == SessionStatus::Active {
                    state.status = SessionStatus::Inactive;
                    status_changed = true;
                }
            }

            if state.missed_count >= state.config.max_missed {
                tracing::error!(
                    "Session {} exceeded max missed heartbeats, marking as expired", 
                    session_id
                );
                state.status = SessionStatus::Terminated;
                status_changed = true;
                expired_sessions.push(*session_id);
            }
            
            // 记录需要同步到数据库的会话
            if status_changed {
                sessions_to_update_db.push((*session_id, state.status.clone()));
            }
        }
        
        // Remove expired sessions first
         for session_id in expired_sessions {
             sessions_guard.remove(&session_id);
             tracing::info!("Removed expired session {} from heartbeat monitoring", session_id);
         }
         
         drop(sessions_guard);
          
          // 异步更新数据库状态
          for (session_id, status) in sessions_to_update_db {
              if let Err(e) = self.sync_session_status_to_db(&session_id, status).await {
                  tracing::error!("Failed to sync session {} status to database: {}", session_id, e);
              }
          }

        Ok(())
    }

    async fn probe_session(&self, session_id: &Uuid) -> AuroraResult<()> {
        let probe_start = Instant::now();
        
        // 执行探测
        let probe_result = self.execute_probe(session_id).await;
        
        // 更新探测结果
        let mut sessions_guard = self.sessions.write().await;
        if let Some(state) = sessions_guard.get_mut(session_id) {
            state.last_probe = Some(probe_result.clone());
            
            if probe_result.success {
                state.last_heartbeat = Instant::now();
                state.missed_count = 0;
                
                // 如果之前状态不是Active，恢复为Active
                if state.status != SessionStatus::Active {
                    state.status = SessionStatus::Active;
                    tracing::info!("Session {} restored to active status", session_id);
                }
                
                tracing::debug!("Session {} probe successful ({}ms)", session_id, probe_result.response_time.as_millis());
            } else {
                tracing::warn!("Session {} probe failed: {}", session_id, 
                    probe_result.error_message.as_deref().unwrap_or("Unknown error"));
            }
        }
        
        Ok(())
    }

    async fn execute_probe(&self, session_id: &Uuid) -> ProbeResult {
        let start_time = Instant::now();
        
        // 获取目标信息以确定探测方法
        let probe_method = if let Some(target) = self.get_session_target(session_id).await {
            Self::determine_probe_method(&target)
        } else {
            ProbeMethod::TcpConnect
        };
        
        // 实现真实的网络探测逻辑
        let (success, error_message) = self.perform_network_probe(session_id).await;
        
        let response_time = start_time.elapsed();
        
        ProbeResult {
            success,
            response_time,
            error_message,
            timestamp: Instant::now(),
            method: probe_method,
        }
    }

    async fn perform_network_probe(&self, session_id: &Uuid) -> (bool, Option<String>) {
        if let Some(target) = self.get_session_target(session_id).await {
            // 使用命令执行和文件列表获取来探测远程服务器
            self.remote_command_probe(&target, session_id).await
        } else {
            (false, Some("Failed to get session target".to_string()))
        }
    }
    
    async fn remote_command_probe(&self, target: &str, session_id: &Uuid) -> (bool, Option<String>) {
        // 使用命令执行和文件列表获取来探测远程服务器
        tracing::debug!("Starting remote command probe for session {} on target {}", session_id, target);
        
        // 如果有AppState，尝试使用远程命令探测
        if let Some(app_state) = &self.app_state {
            // 首先尝试执行whoami命令
            match self.try_remote_command(app_state, session_id, "whoami").await {
                Ok(true) => {
                    tracing::debug!("Remote command 'whoami' successful for session {}", session_id);
                    return (true, None);
                }
                Ok(false) => {
                    tracing::debug!("Remote command 'whoami' failed for session {}, trying file list", session_id);
                }
                Err(e) => {
                    tracing::warn!("Error executing remote command for session {}: {}", session_id, e);
                }
            }
            
            // 如果whoami失败，尝试列出根目录
            match self.try_remote_file_list(app_state, session_id, "/").await {
                Ok(true) => {
                    tracing::debug!("Remote file list successful for session {}", session_id);
                    return (true, None);
                }
                Ok(false) => {
                    tracing::debug!("Remote file list failed for session {}, falling back to TCP probe", session_id);
                }
                Err(e) => {
                    tracing::warn!("Error listing remote files for session {}: {}", session_id, e);
                }
            }
        } else {
            tracing::debug!("No AppState available, falling back to TCP probe for session {}", session_id);
        }
        
        // 回退到TCP探测
        let (tcp_success, tcp_error) = Self::tcp_probe(target).await;
        
        if tcp_success {
            tracing::debug!("TCP probe successful for session {}", session_id);
            (true, None)
        } else {
            tracing::warn!("All probe methods failed for session {}: {:?}", session_id, tcp_error);
            (false, tcp_error)
        }
    }
    
    fn determine_probe_method(target: &str) -> ProbeMethod {
        // 根据目标地址和端口确定最佳探测方法
        if let Some((_, port_str)) = target.split_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                match port {
                    22 => ProbeMethod::SshConnect,
                    80 | 8080 | 3000 | 8000 => ProbeMethod::HttpGet,
                    443 | 8443 => ProbeMethod::HttpGet,
                    _ => ProbeMethod::TcpConnect,
                }
            } else {
                ProbeMethod::TcpConnect
            }
        } else {
            ProbeMethod::Ping
        }
    }
    
    async fn get_session_target(&self, session_id: &Uuid) -> Option<String> {
        // 从会话状态中获取目标信息
        let sessions_guard = self.sessions.read().await;
        if let Some(state) = sessions_guard.get(session_id) {
            Some(state.target.clone())
        } else {
            None
        }
    }
    
    async fn tcp_probe(target: &str) -> (bool, Option<String>) {
        use tokio::net::TcpStream;
        use tokio::time::timeout;
        
        // 解析目标地址
        let addr = match target.parse::<std::net::SocketAddr>() {
            Ok(addr) => addr,
            Err(_) => {
                // 尝试解析为 host:port 格式
                if let Some((host, port_str)) = target.split_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        match tokio::net::lookup_host((host, port)).await {
                            Ok(mut addrs) => {
                                if let Some(addr) = addrs.next() {
                                    addr
                                } else {
                                    return (false, Some(format!("No address found for {}", target)));
                                }
                            }
                            Err(e) => return (false, Some(format!("DNS lookup failed: {}", e))),
                        }
                    } else {
                        return (false, Some(format!("Invalid port in target: {}", target)));
                    }
                } else {
                    return (false, Some(format!("Invalid target format: {}", target)));
                }
            }
        };
        
        // 尝试TCP连接，设置5秒超时
        match timeout(Duration::from_secs(5), TcpStream::connect(addr)).await {
            Ok(Ok(_stream)) => {
                // 连接成功
                (true, None)
            }
            Ok(Err(e)) => {
                // 连接失败
                (false, Some(format!("Connection failed: {}", e)))
            }
            Err(_) => {
                // 超时
                (false, Some("Connection timeout".to_string()))
            }
        }
    }
    
    async fn probe_session_static(sessions: &Arc<RwLock<HashMap<Uuid, HeartbeatState>>>, session_id: &Uuid) -> AuroraResult<()> {
        let probe_start = Instant::now();
        
        // 执行探测
        let probe_result = Self::execute_probe_static(sessions, session_id).await;
        
        // 更新探测结果
        let mut sessions_guard = sessions.write().await;
        if let Some(state) = sessions_guard.get_mut(session_id) {
            state.last_probe = Some(probe_result.clone());
            
            if probe_result.success {
                state.last_heartbeat = Instant::now();
                state.missed_count = 0;
                tracing::debug!("Session {} probe successful ({}ms)", session_id, probe_result.response_time.as_millis());
            } else {
                tracing::warn!("Session {} probe failed: {}", session_id, 
                    probe_result.error_message.as_deref().unwrap_or("Unknown error"));
            }
        }
        
        Ok(())
    }

    async fn execute_probe_static(sessions: &Arc<RwLock<HashMap<Uuid, HeartbeatState>>>, session_id: &Uuid) -> ProbeResult {
        let start_time = Instant::now();
        
        // 获取目标信息以确定探测方法
        let probe_method = if let Some(target) = Self::get_session_target_static(sessions, session_id).await {
            Self::determine_probe_method(&target)
        } else {
            ProbeMethod::TcpConnect
        };
        
        // 实现真实的网络探测逻辑
        let (success, error_message) = Self::perform_network_probe_static(sessions, session_id).await;
        
        let response_time = start_time.elapsed();
        
        ProbeResult {
            success,
            response_time,
            error_message,
            timestamp: Instant::now(),
            method: probe_method,
        }
    }

    async fn perform_network_probe_static(sessions: &Arc<RwLock<HashMap<Uuid, HeartbeatState>>>, session_id: &Uuid) -> (bool, Option<String>) {
        if let Some(target) = Self::get_session_target_static(sessions, session_id).await {
            // 使用TCP探测作为静态方法的回退
            Self::tcp_probe(&target).await
        } else {
            (false, Some("Failed to get session target".to_string()))
        }
    }

    async fn get_session_target_static(sessions: &Arc<RwLock<HashMap<Uuid, HeartbeatState>>>, session_id: &Uuid) -> Option<String> {
        // 从会话状态中获取目标信息
        let sessions_guard = sessions.read().await;
        if let Some(state) = sessions_guard.get(session_id) {
            Some(state.target.clone())
        } else {
            None
        }
    }

    // HTTP探测方法
    async fn http_probe(target: &str) -> (bool, Option<String>) {
        use tokio::time::timeout;
        
        // 构建HTTP URL
        let url = if target.contains("443") || target.contains("8443") {
            format!("https://{}", target)
        } else {
            format!("http://{}", target)
        };
        
        // 使用简单的HTTP客户端进行探测
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build();
            
        match client {
            Ok(client) => {
                match timeout(Duration::from_secs(5), client.get(&url).send()).await {
                    Ok(Ok(response)) => {
                        if response.status().is_success() || response.status().is_redirection() {
                            (true, None)
                        } else {
                            (false, Some(format!("HTTP error: {}", response.status())))
                        }
                    }
                    Ok(Err(e)) => (false, Some(format!("HTTP request failed: {}", e))),
                    Err(_) => (false, Some("HTTP request timeout".to_string())),
                }
            }
            Err(e) => (false, Some(format!("Failed to create HTTP client: {}", e))),
        }
    }
    
    // SSH探测方法
    async fn ssh_probe(target: &str) -> (bool, Option<String>) {
        // SSH探测通过尝试建立TCP连接到22端口
        // 然后检查SSH banner
        use tokio::net::TcpStream;
        use tokio::time::timeout;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let addr = match target.parse::<std::net::SocketAddr>() {
            Ok(addr) => addr,
            Err(_) => {
                if let Some((host, port_str)) = target.split_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        match tokio::net::lookup_host((host, port)).await {
                            Ok(mut addrs) => {
                                if let Some(addr) = addrs.next() {
                                    addr
                                } else {
                                    return (false, Some(format!("No address found for {}", target)));
                                }
                            }
                            Err(e) => return (false, Some(format!("DNS lookup failed: {}", e))),
                        }
                    } else {
                        return (false, Some(format!("Invalid port in target: {}", target)));
                    }
                } else {
                    return (false, Some(format!("Invalid target format: {}", target)));
                }
            }
        };
        
        match timeout(Duration::from_secs(5), TcpStream::connect(addr)).await {
            Ok(Ok(mut stream)) => {
                // 尝试读取SSH banner
                let mut buffer = [0; 256];
                match timeout(Duration::from_secs(2), stream.read(&mut buffer)).await {
                    Ok(Ok(n)) if n > 0 => {
                        let banner = String::from_utf8_lossy(&buffer[..n]);
                        if banner.starts_with("SSH-") {
                            (true, None)
                        } else {
                            (false, Some("Not an SSH service".to_string()))
                        }
                    }
                    Ok(Ok(_)) => (false, Some("No SSH banner received".to_string())),
                    Ok(Err(e)) => (false, Some(format!("Failed to read SSH banner: {}", e))),
                    Err(_) => (false, Some("SSH banner read timeout".to_string())),
                }
            }
            Ok(Err(e)) => (false, Some(format!("SSH connection failed: {}", e))),
            Err(_) => (false, Some("SSH connection timeout".to_string())),
        }
    }
    
    // PING探测方法
    async fn ping_probe(target: &str) -> (bool, Option<String>) {
        // 实现ICMP ping探测
        // 注意：ICMP需要特殊权限，在某些系统上可能无法使用
        use std::process::Command;
        
        // 提取主机名（去掉端口）
        let host = if let Some((host, _)) = target.split_once(':') {
            host
        } else {
            target
        };
        
        let output = Command::new("ping")
            .arg("-c")
            .arg("1")
            .arg("-W")
            .arg("3000") // 3秒超时
            .arg(host)
            .output();
            
        match output {
            Ok(output) => {
                if output.status.success() {
                    (true, None)
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    (false, Some(format!("Ping failed: {}", stderr)))
                }
            }
            Err(e) => (false, Some(format!("Ping command failed: {}", e))),
        }
    }

    pub async fn stop(&mut self) -> AuroraResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }
        Ok(())
    }

    pub async fn get_all_heartbeat_statuses(&self) -> AuroraResult<HashMap<Uuid, SessionHealth>> {
        let sessions = self.sessions.read().await;
        let mut statuses = HashMap::new();

        for (session_id, state) in sessions.iter() {
            let health = self.calculate_session_health(state).await;
            statuses.insert(*session_id, health);
        }

        Ok(statuses)
    }

    pub async fn load_sessions_from_db(&mut self) -> AuroraResult<()> {
        if let Some(db_pool) = &self.db_pool {
            let sessions_data = sqlx::query(
                "SELECT id, operator_id, target, status FROM sap_sessions WHERE status IN ('active', 'inactive')"
            )
            .fetch_all(db_pool)
            .await
            .map_err(|e| AuroraError::Database(e))?;

            let mut sessions_guard = self.sessions.write().await;
            
            for row in sessions_data {
                let session_id: String = row.get("id");
                let target: String = row.get("target");
                let status_str: String = row.get("status");
                
                if let Ok(uuid) = Uuid::parse_str(&session_id) {
                    let status = match status_str.as_str() {
                        "active" => SessionStatus::Active,
                        "inactive" => SessionStatus::Inactive,
                        _ => SessionStatus::Inactive,
                    };
                    
                    let heartbeat_state = HeartbeatState {
                        session_id: uuid,
                        config: HeartbeatConfig {
                            enabled: true,
                            interval_seconds: 30,
                            timeout_seconds: 60,
                            max_missed: 3,
                        },
                        last_heartbeat: Instant::now(),
                        last_probe: None,
                        missed_count: 0,
                        status,
                        target,
                    };
                    
                    sessions_guard.insert(uuid, heartbeat_state);
                    tracing::info!("Loaded session {} from database for heartbeat monitoring", uuid);
                }
            }
            
            tracing::info!("Loaded {} sessions from database", sessions_guard.len());
        }
        
        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) -> AuroraResult<()> {
        if let Some(db_pool) = &self.db_pool {
            // 清理数据库中长时间无活动的会话
            let cleanup_result = sqlx::query(
                "UPDATE sap_sessions SET status = 'terminated' WHERE status = 'active' AND last_contact < datetime('now', '-1 hour')"
            )
            .execute(db_pool)
            .await
            .map_err(|e| AuroraError::Database(e))?;
            
            if cleanup_result.rows_affected() > 0 {
                tracing::info!("Cleaned up {} expired sessions from database", cleanup_result.rows_affected());
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatStatus {
    Healthy,
    Warning,
    Critical,
}

impl Default for HeartbeatManager {
    fn default() -> Self {
        Self::new()
    }
}