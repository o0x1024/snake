// Example usage of the session management system
// This file demonstrates how to use the enhanced session management features

use uuid::Uuid;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use chrono::Utc;

use crate::error::AuroraResult;
use super::*;

pub async fn example_session_workflow() -> AuroraResult<()> {
    // 1. Create session manager with configuration
    let config = SessionConfig {
        timeout_minutes: 60,
        max_concurrent_sessions: 5,
        enable_heartbeat: true,
        heartbeat_interval_seconds: 30,
    };

    // 2. Initialize with persistence and audit logging
    let manager = SessionManager::new(config)
        .with_persistence("sqlite:sessions.db")
        .await?;

    // 3. Start background services
    manager.start_heartbeat_manager().await?;
    manager.start_collaboration_server("127.0.0.1:8080").await?;

    // 4. Create a session with SOCKS5 proxy
    let proxy_config = ProxyConfig {
        proxy_type: ProxyType::Socks5,
        address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1080),
        username: Some("proxy_user".to_string()),
        password: Some("proxy_pass".to_string()),
    };

    let session_id = manager.create_session(
        "operator_001".to_string(),
        "192.168.1.100:22".to_string(),
        Some(proxy_config),
    ).await?;

    println!("Created session: {}", session_id);

    // 5. Simulate command execution with audit logging
    manager.log_command_execution(
        &session_id,
        "operator_001",
        "whoami",
        Some("root"),
    ).await?;

    manager.log_command_execution(
        &session_id,
        "operator_001",
        "ls -la /home",
        Some("drwxr-xr-x 3 user user 4096 Jan 1 12:00 user1\ndrwxr-xr-x 3 user user 4096 Jan 1 12:00 user2"),
    ).await?;

    // 6. Simulate file operations
    manager.log_file_access(
        &session_id,
        "operator_001",
        "/etc/passwd",
        "read",
    ).await?;

    manager.log_file_access(
        &session_id,
        "operator_001",
        "/tmp/test.txt",
        "write",
    ).await?;

    // 7. Send collaboration messages
    let status_message = CollaborationMessage {
        id: Uuid::new_v4(),
        session_id,
        operator_id: "operator_001".to_string(),
        message_type: MessageType::Status,
        content: "Successfully connected to target".to_string(),
        timestamp: Utc::now(),
    };

    manager.broadcast_message(&session_id, status_message).await?;

    let alert_message = CollaborationMessage {
        id: Uuid::new_v4(),
        session_id,
        operator_id: "operator_001".to_string(),
        message_type: MessageType::Alert,
        content: "Detected antivirus software".to_string(),
        timestamp: Utc::now(),
    };

    manager.broadcast_message(&session_id, alert_message).await?;

    // 8. Update session activity (simulating heartbeat)
    manager.update_activity(&session_id).await?;

    // 9. Check heartbeat status
    let heartbeat_status = manager.get_heartbeat_status(&session_id).await?;
    println!("Heartbeat status: {:?}", heartbeat_status);

    // 10. Get audit logs for the session
    let audit_logs = manager.get_session_audit_logs(&session_id, Some(10), None).await?;
    println!("Session has {} audit log entries", audit_logs.len());

    for log in &audit_logs {
        println!("  - {:?}: {} (Risk: {:?})", 
                 log.action, 
                 log.details.as_deref().unwrap_or("N/A"),
                 log.risk_level);
    }

    // 11. Get high-risk activities from the last 24 hours
    let high_risk_logs = manager.get_high_risk_logs(Some(24), Some(5)).await?;
    if !high_risk_logs.is_empty() {
        println!("High-risk activities detected:");
        for log in &high_risk_logs {
            println!("  - {}: {:?} by {} (Risk: {:?})", 
                     log.timestamp.format("%Y-%m-%d %H:%M:%S"),
                     log.action,
                     log.operator_id,
                     log.risk_level);
        }
    }

    // 12. Get audit summary
    let summary = manager.get_audit_summary(Some(session_id), None, Some(1)).await?;
    if let Some(summary_entry) = summary.first() {
        println!("Today's activity summary:");
        println!("  - Total actions: {}", summary_entry.total_actions);
        println!("  - High-risk actions: {}", summary_entry.high_risk_actions);
        println!("  - Critical actions: {}", summary_entry.critical_actions);
    }

    // 13. Send data through proxy (if available)
    let test_data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    if let Err(e) = manager.send_data_through_proxy(&session_id, test_data).await {
        println!("Proxy communication failed (expected if no proxy server): {}", e);
    }

    // 14. List all active sessions
    let active_sessions = manager.list_active_sessions().await?;
    println!("Active sessions: {}", active_sessions.len());

    // 15. Get session collaborators
    let collaborators = manager.get_session_collaborators(&session_id).await?;
    println!("Session collaborators: {}", collaborators.len());

    // 16. Cleanup expired sessions
    let expired = manager.cleanup_expired_sessions().await?;
    if !expired.is_empty() {
        println!("Cleaned up {} expired sessions", expired.len());
    }

    // 17. Terminate the session
    manager.terminate_session(&session_id).await?;
    println!("Session terminated: {}", session_id);

    // 18. Shutdown the manager
    manager.shutdown().await?;
    println!("Session manager shut down");

    Ok(())
}

pub async fn example_collaboration_scenario() -> AuroraResult<()> {
    println!("=== Collaboration Scenario Example ===");

    let config = SessionConfig {
        timeout_minutes: 120,
        max_concurrent_sessions: 10,
        enable_heartbeat: true,
        heartbeat_interval_seconds: 60,
    };

    let manager = SessionManager::new(config)
        .with_persistence("sqlite:collaboration_demo.db")
        .await?;

    manager.start_collaboration_server("127.0.0.1:8081").await?;

    // Create a session for a penetration testing scenario
    let session_id = manager.create_session(
        "lead_operator".to_string(),
        "target.company.com".to_string(),
        None,
    ).await?;

    // Simulate multiple operators working together
    let operators = vec!["lead_operator", "junior_operator", "observer"];

    for operator in &operators {
        // Each operator performs different actions
        match *operator {
            "lead_operator" => {
                manager.log_command_execution(
                    &session_id,
                    operator,
                    "nmap -sS target.company.com",
                    Some("22/tcp open ssh\n80/tcp open http\n443/tcp open https"),
                ).await?;

                let message = CollaborationMessage {
                    id: Uuid::new_v4(),
                    session_id,
                    operator_id: operator.to_string(),
                    message_type: MessageType::Status,
                    content: "Initial reconnaissance complete. Found SSH, HTTP, HTTPS".to_string(),
                    timestamp: Utc::now(),
                };
                manager.broadcast_message(&session_id, message).await?;
            }
            "junior_operator" => {
                manager.log_command_execution(
                    &session_id,
                    operator,
                    "gobuster dir -u http://target.company.com -w wordlist.txt",
                    Some("/admin\n/login\n/api"),
                ).await?;

                let message = CollaborationMessage {
                    id: Uuid::new_v4(),
                    session_id,
                    operator_id: operator.to_string(),
                    message_type: MessageType::Status,
                    content: "Directory enumeration found /admin, /login, /api endpoints".to_string(),
                    timestamp: Utc::now(),
                };
                manager.broadcast_message(&session_id, message).await?;
            }
            "observer" => {
                let message = CollaborationMessage {
                    id: Uuid::new_v4(),
                    session_id,
                    operator_id: operator.to_string(),
                    message_type: MessageType::Chat,
                    content: "Monitoring progress. All activities logged for compliance.".to_string(),
                    timestamp: Utc::now(),
                };
                manager.broadcast_message(&session_id, message).await?;
            }
            _ => {}
        }
    }

    // Simulate a critical finding
    manager.log_command_execution(
        &session_id,
        "lead_operator",
        "sqlmap -u 'http://target.company.com/login' --dbs",
        Some("available databases [3]:\n[*] information_schema\n[*] mysql\n[*] webapp"),
    ).await?;

    let alert_message = CollaborationMessage {
        id: Uuid::new_v4(),
        session_id,
        operator_id: "lead_operator".to_string(),
        message_type: MessageType::Alert,
        content: "SQL injection vulnerability confirmed! Database access obtained.".to_string(),
        timestamp: Utc::now(),
    };
    manager.broadcast_message(&session_id, alert_message).await?;

    // Generate final report data
    let audit_logs = manager.get_session_audit_logs(&session_id, None, None).await?;
    let high_risk_logs = manager.get_high_risk_logs(Some(1), None).await?;

    println!("Collaboration session completed:");
    println!("  - Total audit entries: {}", audit_logs.len());
    println!("  - High-risk activities: {}", high_risk_logs.len());

    manager.terminate_session(&session_id).await?;
    manager.shutdown().await?;

    Ok(())
}

#[cfg(test)]
mod example_tests {
    use super::*;

    #[tokio::test]
    async fn test_example_workflow() {
        // This test ensures the example code compiles and basic functionality works
        // In a real scenario, you'd want to mock external dependencies
        
        let config = SessionConfig {
            timeout_minutes: 1,
            max_concurrent_sessions: 1,
            enable_heartbeat: false,
            heartbeat_interval_seconds: 30,
        };

        let manager = SessionManager::new(config);
        
        let session_id = manager.create_session(
            "test_operator".to_string(),
            "127.0.0.1".to_string(),
            None,
        ).await.expect("Failed to create session");

        let session = manager.get_session(&session_id).await.expect("Failed to get session");
        assert_eq!(session.operator_id, "test_operator");
        
        manager.terminate_session(&session_id).await.expect("Failed to terminate session");
    }
}