// Session management module
pub mod manager;
pub mod types;
pub mod persistence;
pub mod heartbeat;
pub mod proxy;
pub mod collaboration;
pub mod audit;

#[cfg(test)]
mod tests;

pub mod example;

pub use manager::SessionManager;
pub use types::*;
pub use persistence::{SessionPersistence, SessionLogEntry};
pub use heartbeat::{HeartbeatManager, HeartbeatStatus};
pub use proxy::{ProxyConnector, ProxyTunnel};
pub use collaboration::{CollaborationManager, CollaborationMessage, MessageType, CollaboratorInfo, CollaboratorRole};
pub use audit::{AuditManager, AuditAction, AuditLog, AuditSummary, RiskLevel};