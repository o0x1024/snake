use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::Utc;

use crate::error::{AuroraResult, SessionError};
use super::types::{Session, SessionConfig, SessionStatus, ProxyConfig, HeartbeatConfig};
use super::persistence::{SessionPersistence, SessionLogEntry};
use super::heartbeat::{HeartbeatManager, HeartbeatStatus, SessionHealth};
use super::proxy::ProxyTunnel;
use super::collaboration::{CollaborationManager, CollaborationMessage, MessageType, CollaboratorInfo};
use super::audit::{AuditManager, AuditAction, AuditLog, AuditSummary};

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
    config: SessionConfig,
    persistence: Option<SessionPersistence>,
    heartbeat_manager: Arc<RwLock<HeartbeatManager>>,
    proxy_tunnels: Arc<RwLock<HashMap<Uuid, ProxyTunnel>>>,
    collaboration_manager: Arc<RwLock<CollaborationManager>>,
    audit_manager: Option<AuditManager>,
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
            persistence: None,
            heartbeat_manager: Arc::new(RwLock::new(HeartbeatManager::new())),
            proxy_tunnels: Arc::new(RwLock::new(HashMap::new())),
            collaboration_manager: Arc::new(RwLock::new(CollaborationManager::new())),
            audit_manager: None,
        }
    }

    pub async fn with_persistence(mut self, database_url: &str) -> AuroraResult<Self> {
        let persistence = SessionPersistence::new(database_url).await?;
        let audit_manager = AuditManager::new(database_url).await?;
        
        // Load existing sessions from database
        let stored_sessions = persistence.load_all_sessions().await?;
        
        {
            let mut sessions = self.sessions.write().await;
            
            for session in stored_sessions {
                // Register with heartbeat manager if enabled
                if session.heartbeat_config.enabled {
                    let heartbeat_manager = self.heartbeat_manager.write().await;
                    heartbeat_manager.register_session(&session).await?;
                }
                
                // Create collaboration broadcast for existing session
                let collaboration_manager = self.collaboration_manager.read().await;
                collaboration_manager.create_session_broadcast(session.id).await?;
                
                sessions.insert(session.id, session);
            }
        }
        
        self.persistence = Some(persistence);
        self.audit_manager = Some(audit_manager);
        Ok(self)
    }

    pub async fn start_heartbeat_manager(&self) -> AuroraResult<()> {
        let mut heartbeat_manager = self.heartbeat_manager.write().await;
        heartbeat_manager.start().await
    }

    /// Load existing sessions from database and register them to heartbeat manager
    pub async fn load_sessions_from_db(&self) -> AuroraResult<usize> {
        if let Some(persistence) = &self.persistence {
            // Load active sessions from database
            let db_sessions = persistence.get_active_sessions().await?;
            let mut loaded_count = 0;

            for session in db_sessions {
                // Add to memory
                {
                    let mut sessions = self.sessions.write().await;
                    sessions.insert(session.id, session.clone());
                }

                // Register to heartbeat manager
                {
                    let heartbeat_manager = self.heartbeat_manager.read().await;
                    heartbeat_manager.register_session(&session).await?;
                }

                loaded_count += 1;
            }

            Ok(loaded_count)
        } else {
            Ok(0)
        }
    }

    /// Initialize session manager with database loading
    pub async fn initialize(&self) -> AuroraResult<()> {
        // Start heartbeat manager
        self.start_heartbeat_manager().await?;
        
        // Load existing sessions from database
        let loaded_count = self.load_sessions_from_db().await?;
        
        tracing::info!("Loaded {} sessions from database", loaded_count);
        
        Ok(())
    }

    pub async fn start_collaboration_server(&self, bind_addr: &str) -> AuroraResult<()> {
        let mut collaboration_manager = self.collaboration_manager.write().await;
        collaboration_manager.start_server(bind_addr).await?;
        Ok(())
    }

    pub async fn create_session(
        &self,
        operator_id: String,
        target: String,
        proxy_config: Option<ProxyConfig>,
    ) -> AuroraResult<Uuid> {
        let session_id = Uuid::new_v4();
        
        let heartbeat_config = HeartbeatConfig {
            enabled: self.config.enable_heartbeat,
            interval_seconds: self.config.heartbeat_interval_seconds,
            timeout_seconds: self.config.heartbeat_interval_seconds * 3, // 3x interval as timeout
            max_missed: 3,
        };

        let session = Session {
            id: session_id,
            operator_id: operator_id.clone(),
            target: target.clone(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
            status: SessionStatus::Active,
            proxy_config: proxy_config.clone(),
            heartbeat_config,
        };

        let mut sessions = self.sessions.write().await;
        
        // Check session limit
        let active_count = sessions.values()
            .filter(|s| matches!(s.status, SessionStatus::Active))
            .count();
            
        if active_count >= self.config.max_concurrent_sessions as usize {
            return Err(SessionError::LimitExceeded.into());
        }

        // Save to persistence if available
        if let Some(persistence) = &self.persistence {
            persistence.save_session(&session).await?;
            persistence.log_session_event(
                &session_id,
                "session_created",
                Some(&format!("operator: {}, target: {}", operator_id, target)),
            ).await?;
        }

        // Log audit event
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.log_action(
                session_id,
                &operator_id,
                AuditAction::SessionCreated,
                Some(&target),
                Some(&format!("proxy_enabled: {}", proxy_config.is_some())),
                None,
                None,
            ).await?;
        }

        // Create collaboration broadcast channel
        let collaboration_manager = self.collaboration_manager.read().await;
        collaboration_manager.create_session_broadcast(session_id).await?;

        // Register with heartbeat manager if enabled
        if session.heartbeat_config.enabled {
            let heartbeat_manager = self.heartbeat_manager.write().await;
            heartbeat_manager.register_session(&session).await?;
        }

        // Establish proxy tunnel if configured
        if let Some(proxy_config) = proxy_config {
            if let Ok(target_addr) = target.parse() {
                match ProxyTunnel::establish(proxy_config, target_addr).await {
                    Ok(tunnel) => {
                        let mut proxy_tunnels = self.proxy_tunnels.write().await;
                        proxy_tunnels.insert(session_id, tunnel);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to establish proxy tunnel for session {}: {}", session_id, e);
                    }
                }
            }
        }

        sessions.insert(session_id, session);
        Ok(session_id)
    }

    pub async fn get_session(&self, session_id: &Uuid) -> AuroraResult<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()).into())
    }

    pub async fn terminate_session(&self, session_id: &Uuid) -> AuroraResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = SessionStatus::Terminated;
            session.last_activity = Utc::now();

            // Save to persistence if available
            if let Some(persistence) = &self.persistence {
                persistence.save_session(session).await?;
                persistence.log_session_event(
                    session_id,
                    "session_terminated",
                    None,
                ).await?;
            }

            // Unregister from heartbeat manager
            let heartbeat_manager = self.heartbeat_manager.read().await;
            heartbeat_manager.unregister_session(session_id).await?;

            // Close proxy tunnel if exists
            let mut proxy_tunnels = self.proxy_tunnels.write().await;
            if let Some(tunnel) = proxy_tunnels.remove(session_id) {
                let _ = tunnel.close().await; // Ignore errors on close
            }

            Ok(())
        } else {
            Err(SessionError::NotFound(session_id.to_string()).into())
        }
    }

    pub async fn update_activity(&self, session_id: &Uuid) -> AuroraResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_activity = Utc::now();

            // Update heartbeat
            let heartbeat_manager = self.heartbeat_manager.read().await;
            heartbeat_manager.update_heartbeat(session_id).await?;

            // Save to persistence if available
            if let Some(persistence) = &self.persistence {
                persistence.save_session(session).await?;
            }

            Ok(())
        } else {
            Err(SessionError::NotFound(session_id.to_string()).into())
        }
    }

    pub async fn list_active_sessions(&self) -> AuroraResult<Vec<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values()
            .filter(|s| matches!(s.status, SessionStatus::Active))
            .cloned()
            .collect())
    }

    pub async fn get_session_logs(&self, session_id: &Uuid) -> AuroraResult<Vec<SessionLogEntry>> {
        if let Some(persistence) = &self.persistence {
            persistence.get_session_logs(session_id).await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_heartbeat_status(&self, session_id: &Uuid) -> AuroraResult<SessionHealth> {
        let heartbeat_manager = self.heartbeat_manager.read().await;
        heartbeat_manager.get_session_heartbeat_status(session_id).await
    }

    pub async fn get_all_heartbeat_statuses(&self) -> AuroraResult<HashMap<Uuid, SessionHealth>> {
        let heartbeat_manager = self.heartbeat_manager.read().await;
        heartbeat_manager.get_all_heartbeat_statuses().await
    }

    pub async fn send_data_through_proxy(&self, session_id: &Uuid, data: &[u8]) -> AuroraResult<()> {
        let mut proxy_tunnels = self.proxy_tunnels.write().await;
        if let Some(tunnel) = proxy_tunnels.get_mut(session_id) {
            tunnel.send_data(data).await?;
            
            // Log the activity
            if let Some(persistence) = &self.persistence {
                persistence.log_session_event(
                    session_id,
                    "data_sent",
                    Some(&format!("bytes: {}", data.len())),
                ).await?;
            }

            // Update activity
            self.update_activity(session_id).await?;
            Ok(())
        } else {
            Err(SessionError::NotFound(session_id.to_string()).into())
        }
    }

    pub async fn receive_data_through_proxy(&self, session_id: &Uuid, buffer: &mut [u8]) -> AuroraResult<usize> {
        let mut proxy_tunnels = self.proxy_tunnels.write().await;
        if let Some(tunnel) = proxy_tunnels.get_mut(session_id) {
            let bytes_received = tunnel.receive_data(buffer).await?;
            
            // Log the activity
            if let Some(persistence) = &self.persistence {
                persistence.log_session_event(
                    session_id,
                    "data_received",
                    Some(&format!("bytes: {}", bytes_received)),
                ).await?;
            }

            // Update activity
            self.update_activity(session_id).await?;
            Ok(bytes_received)
        } else {
            Err(SessionError::NotFound(session_id.to_string()).into())
        }
    }

    pub async fn cleanup_expired_sessions(&self) -> AuroraResult<Vec<Uuid>> {
        let mut sessions = self.sessions.write().await;
        let mut expired_sessions = Vec::new();
        let timeout_duration = chrono::Duration::minutes(self.config.timeout_minutes as i64);

        for (session_id, session) in sessions.iter_mut() {
            if matches!(session.status, SessionStatus::Active) {
                let elapsed = Utc::now() - session.last_activity;
                if elapsed > timeout_duration {
                    session.status = SessionStatus::Terminated;
                    expired_sessions.push(*session_id);

                    // Log expiration
                    if let Some(persistence) = &self.persistence {
                        let _ = persistence.save_session(session).await;
                        let _ = persistence.log_session_event(
                            session_id,
                            "session_expired",
                            Some(&format!("inactive_for_minutes: {}", elapsed.num_minutes())),
                        ).await;
                    }
                }
            }
        }

        // Clean up expired sessions
        for session_id in &expired_sessions {
            // Unregister from heartbeat manager
            let heartbeat_manager = self.heartbeat_manager.read().await;
            let _ = heartbeat_manager.unregister_session(session_id).await;

            // Close proxy tunnel if exists
            let mut proxy_tunnels = self.proxy_tunnels.write().await;
            if let Some(tunnel) = proxy_tunnels.remove(session_id) {
                let _ = tunnel.close().await;
            }
        }

        Ok(expired_sessions)
    }

    /// Manually refresh session status from heartbeat manager and database
    pub async fn refresh_session_status(&self, session_id: &Uuid) -> AuroraResult<SessionHealth> {
        // Get status from heartbeat manager
        let health_status = self.get_heartbeat_status(session_id).await?;
        
        // Update session status in memory based on health
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                let new_status = match health_status {
                    SessionHealth::Healthy | SessionHealth::Warning => SessionStatus::Active,
                    SessionHealth::Critical => SessionStatus::Inactive,
                    SessionHealth::Unreachable | SessionHealth::Unknown => SessionStatus::Terminated,
                };
                
                if session.status != new_status {
                    session.status = new_status.clone();
                    session.last_activity = Utc::now();
                    
                    // Save to persistence
                    if let Some(persistence) = &self.persistence {
                        let _ = persistence.save_session(session).await;
                        let _ = persistence.log_session_event(
                            session_id,
                            "status_refreshed",
                            Some(&format!("new_status: {:?}, health: {:?}", new_status, health_status)),
                        ).await;
                    }
                }
            }
        }
        
        Ok(health_status)
    }

    /// Refresh all session statuses
    pub async fn refresh_all_session_statuses(&self) -> AuroraResult<HashMap<Uuid, SessionHealth>> {
        let session_ids: Vec<Uuid> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };
        
        let mut results = HashMap::new();
        
        for session_id in session_ids {
            match self.refresh_session_status(&session_id).await {
                Ok(health) => {
                    results.insert(session_id, health);
                }
                Err(e) => {
                    tracing::warn!("Failed to refresh status for session {}: {}", session_id, e);
                }
            }
        }
        
        Ok(results)
    }

    /// Cleanup inactive sessions from database
    pub async fn cleanup_database_sessions(&self, _inactive_hours: i64) -> AuroraResult<usize> {
        if let Some(_persistence) = &self.persistence {
            // Get heartbeat manager to sync database
            let heartbeat_manager = self.heartbeat_manager.read().await;
            heartbeat_manager.cleanup_expired_sessions().await?;
            
            tracing::info!("Database cleanup completed");
            Ok(1) // Return count of cleanup operations performed
        } else {
            Ok(0)
        }
    }

    pub async fn shutdown(&self) -> AuroraResult<()> {
        // Stop heartbeat manager
        let mut heartbeat_manager = self.heartbeat_manager.write().await;
        heartbeat_manager.stop().await?;

        // Stop collaboration manager
        let mut collaboration_manager = self.collaboration_manager.write().await;
        collaboration_manager.shutdown().await?;

        // Close all proxy tunnels
        let mut proxy_tunnels = self.proxy_tunnels.write().await;
        for (_, tunnel) in proxy_tunnels.drain() {
            let _ = tunnel.close().await;
        }

        // Terminate all active sessions
        let mut sessions = self.sessions.write().await;
        for (session_id, session) in sessions.iter_mut() {
            if matches!(session.status, SessionStatus::Active) {
                session.status = SessionStatus::Terminated;
                session.last_activity = Utc::now();

                if let Some(persistence) = &self.persistence {
                    let _ = persistence.save_session(session).await;
                    let _ = persistence.log_session_event(
                        session_id,
                        "session_shutdown",
                        None,
                    ).await;
                }

                // Log audit event
                if let Some(audit_manager) = &self.audit_manager {
                    let _ = audit_manager.log_action(
                        *session_id,
                        &session.operator_id,
                        AuditAction::SessionTerminated,
                        None,
                        Some("System shutdown"),
                        None,
                        None,
                    ).await;
                }
            }
        }

        Ok(())
    }

    // Collaboration methods
    pub async fn broadcast_message(&self, session_id: &Uuid, message: CollaborationMessage) -> AuroraResult<()> {
        let collaboration_manager = self.collaboration_manager.read().await;
        collaboration_manager.broadcast_message(session_id, message).await
    }

    pub async fn get_session_collaborators(&self, session_id: &Uuid) -> AuroraResult<Vec<CollaboratorInfo>> {
        let collaboration_manager = self.collaboration_manager.read().await;
        collaboration_manager.get_session_collaborators(session_id).await
    }

    pub async fn send_to_collaborator(
        &self,
        session_id: &Uuid,
        operator_id: &str,
        message: CollaborationMessage,
    ) -> AuroraResult<()> {
        let collaboration_manager = self.collaboration_manager.read().await;
        collaboration_manager.send_to_collaborator(session_id, operator_id, message).await
    }

    // Audit methods
    pub async fn log_command_execution(
        &self,
        session_id: &Uuid,
        operator_id: &str,
        command: &str,
        result: Option<&str>,
    ) -> AuroraResult<()> {
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.log_action(
                *session_id,
                operator_id,
                AuditAction::CommandExecuted,
                Some(command),
                result,
                None,
                None,
            ).await?;
        }

        // Also broadcast to collaborators (ignore errors if no collaborators)
        let message = CollaborationMessage {
            id: Uuid::new_v4(),
            session_id: *session_id,
            operator_id: operator_id.to_string(),
            message_type: MessageType::Command,
            content: command.to_string(),
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_message(session_id, message).await;
        Ok(())
    }

    pub async fn log_file_access(
        &self,
        session_id: &Uuid,
        operator_id: &str,
        file_path: &str,
        action: &str,
    ) -> AuroraResult<()> {
        if let Some(audit_manager) = &self.audit_manager {
            let audit_action = match action {
                "read" => AuditAction::FileAccessed,
                "write" | "modify" => AuditAction::FileModified,
                "delete" => AuditAction::FileDeleted,
                _ => AuditAction::FileAccessed,
            };

            audit_manager.log_action(
                *session_id,
                operator_id,
                audit_action,
                Some(file_path),
                Some(action),
                None,
                None,
            ).await?;
        }

        // Broadcast file operation to collaborators (ignore errors if no collaborators)
        let message = CollaborationMessage {
            id: Uuid::new_v4(),
            session_id: *session_id,
            operator_id: operator_id.to_string(),
            message_type: MessageType::Status,
            content: format!("File {}: {}", action, file_path),
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_message(session_id, message).await;
        Ok(())
    }

    pub async fn get_session_audit_logs(
        &self,
        session_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AuroraResult<Vec<AuditLog>> {
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.get_session_audit_logs(session_id, limit, offset).await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_operator_audit_logs(
        &self,
        operator_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AuroraResult<Vec<AuditLog>> {
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.get_operator_audit_logs(operator_id, limit, offset).await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_high_risk_logs(
        &self,
        hours: Option<i64>,
        limit: Option<i64>,
    ) -> AuroraResult<Vec<AuditLog>> {
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.get_high_risk_logs(hours, limit).await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_audit_summary(
        &self,
        session_id: Option<Uuid>,
        operator_id: Option<&str>,
        days: Option<i64>,
    ) -> AuroraResult<Vec<AuditSummary>> {
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.get_audit_summary(session_id, operator_id, days).await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn cleanup_old_audit_logs(&self, days_to_keep: i64) -> AuroraResult<i64> {
        if let Some(audit_manager) = &self.audit_manager {
            audit_manager.cleanup_old_logs(days_to_keep).await
        } else {
            Ok(0)
        }
    }
}