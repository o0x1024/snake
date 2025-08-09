use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio_tungstenite::{tungstenite::Message};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::error::{AuroraResult, NetworkError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub operator_id: String,
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Command,
    Result,
    Alert,
    Status,
    Chat,
    FileTransfer,
    ScreenShare,
}

#[derive(Debug, Clone)]
pub struct CollaboratorInfo {
    pub operator_id: String,
    pub session_id: Uuid,
    pub connected_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub role: CollaboratorRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CollaboratorRole {
    Leader,
    Operator,
    Observer,
}

pub struct CollaborationManager {
    // WebSocket connections for each session
    connections: Arc<RwLock<HashMap<Uuid, Vec<WebSocketConnection>>>>,
    // Broadcast channels for each session
    broadcasters: Arc<RwLock<HashMap<Uuid, broadcast::Sender<CollaborationMessage>>>>,
    // Active collaborators
    collaborators: Arc<RwLock<HashMap<Uuid, Vec<CollaboratorInfo>>>>,
    // Server handle
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
struct WebSocketConnection {
    operator_id: String,
    role: CollaboratorRole,
    sender: tokio::sync::mpsc::UnboundedSender<Message>,
}

impl CollaborationManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            broadcasters: Arc::new(RwLock::new(HashMap::new())),
            collaborators: Arc::new(RwLock::new(HashMap::new())),
            server_handle: None,
        }
    }

    pub async fn start_server(&mut self, bind_addr: &str) -> AuroraResult<()> {
        let listener = TcpListener::bind(bind_addr).await
            .map_err(|_| NetworkError::ConnectionFailed)?;

        let connections = Arc::clone(&self.connections);
        let broadcasters = Arc::clone(&self.broadcasters);
        let collaborators = Arc::clone(&self.collaborators);
        let bind_addr_owned = bind_addr.to_string();

        let handle = tokio::spawn(async move {
            tracing::info!("Collaboration WebSocket server started on {}", bind_addr_owned);

            while let Ok((stream, addr)) = listener.accept().await {
                tracing::info!("New WebSocket connection from {}", addr);
                
                let connections_clone = Arc::clone(&connections);
                let broadcasters_clone = Arc::clone(&broadcasters);
                let collaborators_clone = Arc::clone(&collaborators);

                tokio::spawn(async move {
                    if let Err(e) = Self::handle_connection(
                        stream,
                        connections_clone,
                        broadcasters_clone,
                        collaborators_clone,
                    ).await {
                        tracing::error!("WebSocket connection error: {}", e);
                    }
                });
            }
        });

        self.server_handle = Some(handle);
        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        connections: Arc<RwLock<HashMap<Uuid, Vec<WebSocketConnection>>>>,
        broadcasters: Arc<RwLock<HashMap<Uuid, broadcast::Sender<CollaborationMessage>>>>,
        collaborators: Arc<RwLock<HashMap<Uuid, Vec<CollaboratorInfo>>>>,
    ) -> AuroraResult<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await
            .map_err(|e| NetworkError::Transport(e.to_string()))?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // Handle outgoing messages
        let sender_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if ws_sender.send(message).await.is_err() {
                    break;
                }
            }
        });

        // Handle incoming messages
        let mut session_id: Option<Uuid> = None;
        let mut operator_id: Option<String> = None;
        let mut role: Option<CollaboratorRole> = None;

        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(auth_msg) = serde_json::from_str::<AuthMessage>(&text) {
                        // Handle authentication
                        session_id = Some(auth_msg.session_id);
                        operator_id = Some(auth_msg.operator_id.clone());
                        role = Some(auth_msg.role.clone());

                        // Register connection
                        let mut connections_guard = connections.write().await;
                        let session_connections = connections_guard
                            .entry(auth_msg.session_id)
                            .or_insert_with(Vec::new);

                        session_connections.push(WebSocketConnection {
                            operator_id: auth_msg.operator_id.clone(),
                            role: auth_msg.role.clone(),
                            sender: tx.clone(),
                        });

                        // Register collaborator
                        let mut collaborators_guard = collaborators.write().await;
                        let session_collaborators = collaborators_guard
                            .entry(auth_msg.session_id)
                            .or_insert_with(Vec::new);

                        session_collaborators.push(CollaboratorInfo {
                            operator_id: auth_msg.operator_id.clone(),
                            session_id: auth_msg.session_id,
                            connected_at: Utc::now(),
                            last_activity: Utc::now(),
                            role: auth_msg.role,
                        });

                        // Subscribe to broadcast channel
                        let broadcasters_guard = broadcasters.read().await;
                        if let Some(broadcaster) = broadcasters_guard.get(&auth_msg.session_id) {
                            let mut receiver = broadcaster.subscribe();
                            let tx_clone = tx.clone();

                            tokio::spawn(async move {
                                while let Ok(msg) = receiver.recv().await {
                                    let json_msg = serde_json::to_string(&msg).unwrap_or_default();
                                    if tx_clone.send(Message::Text(json_msg.into())).is_err() {
                                        break;
                                    }
                                }
                            });
                        }

                        // Send authentication success
                        let response = AuthResponse {
                            success: true,
                            message: "Authentication successful".to_string(),
                        };
                        let response_json = serde_json::to_string(&response).unwrap_or_default();
                        let _ = tx.send(Message::Text(response_json.into()));

                    } else if let Ok(collab_msg) = serde_json::from_str::<CollaborationMessage>(&text) {
                        // Handle collaboration message
                        if let Some(sid) = session_id {
                            let broadcasters_guard = broadcasters.read().await;
                            if let Some(broadcaster) = broadcasters_guard.get(&sid) {
                                let _ = broadcaster.send(collab_msg);
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }

        // Cleanup on disconnect
        if let (Some(sid), Some(oid)) = (session_id, operator_id) {
            // Remove connection
            let mut connections_guard = connections.write().await;
            if let Some(session_connections) = connections_guard.get_mut(&sid) {
                session_connections.retain(|conn| conn.operator_id != oid);
                if session_connections.is_empty() {
                    connections_guard.remove(&sid);
                }
            }

            // Remove collaborator
            let mut collaborators_guard = collaborators.write().await;
            if let Some(session_collaborators) = collaborators_guard.get_mut(&sid) {
                session_collaborators.retain(|collab| collab.operator_id != oid);
                if session_collaborators.is_empty() {
                    collaborators_guard.remove(&sid);
                }
            }
        }

        sender_task.abort();
        Ok(())
    }

    pub async fn create_session_broadcast(&self, session_id: Uuid) -> AuroraResult<()> {
        let (tx, _) = broadcast::channel(1000);
        let mut broadcasters = self.broadcasters.write().await;
        broadcasters.insert(session_id, tx);
        Ok(())
    }

    pub async fn broadcast_message(&self, session_id: &Uuid, message: CollaborationMessage) -> AuroraResult<()> {
        let broadcasters = self.broadcasters.read().await;
        if let Some(broadcaster) = broadcasters.get(session_id) {
            broadcaster.send(message)
                .map_err(|_| NetworkError::Transport("Broadcast failed".to_string()))?;
        }
        Ok(())
    }

    pub async fn get_session_collaborators(&self, session_id: &Uuid) -> AuroraResult<Vec<CollaboratorInfo>> {
        let collaborators = self.collaborators.read().await;
        Ok(collaborators.get(session_id).cloned().unwrap_or_default())
    }

    pub async fn remove_session(&self, session_id: &Uuid) -> AuroraResult<()> {
        // Remove broadcast channel
        let mut broadcasters = self.broadcasters.write().await;
        broadcasters.remove(session_id);

        // Remove connections
        let mut connections = self.connections.write().await;
        connections.remove(session_id);

        // Remove collaborators
        let mut collaborators = self.collaborators.write().await;
        collaborators.remove(session_id);

        Ok(())
    }

    pub async fn send_to_collaborator(
        &self,
        session_id: &Uuid,
        operator_id: &str,
        message: CollaborationMessage,
    ) -> AuroraResult<()> {
        let connections = self.connections.read().await;
        if let Some(session_connections) = connections.get(session_id) {
            for conn in session_connections {
                if conn.operator_id == operator_id {
                    let json_msg = serde_json::to_string(&message).unwrap_or_default();
                    conn.sender.send(Message::Text(json_msg.into()))
                        .map_err(|_| NetworkError::Transport("Failed to send message".to_string()))?;
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> AuroraResult<()> {
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }

        // Clear all data
        self.connections.write().await.clear();
        self.broadcasters.write().await.clear();
        self.collaborators.write().await.clear();

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthMessage {
    session_id: Uuid,
    operator_id: String,
    role: CollaboratorRole,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse {
    success: bool,
    message: String,
}

impl Default for CollaborationManager {
    fn default() -> Self {
        Self::new()
    }
}