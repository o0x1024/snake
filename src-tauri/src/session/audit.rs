use sqlx::{SqlitePool, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json;

use crate::error::{AuroraResult, AuroraError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: i64,
    pub session_id: Uuid,
    pub operator_id: String,
    pub action: AuditAction,
    pub resource: Option<String>,
    pub details: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    SessionCreated,
    SessionTerminated,
    CommandExecuted,
    FileAccessed,
    FileModified,
    FileDeleted,
    DataExfiltrated,
    PrivilegeEscalated,
    NetworkConnection,
    ProxyUsed,
    HeartbeatMissed,
    AuthenticationFailed,
    UnauthorizedAccess,
    ComplianceViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

pub struct AuditManager {
    pool: SqlitePool,
}

impl AuditManager {
    pub async fn new(database_url: &str) -> AuroraResult<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        
        // Create audit_logs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                operator_id TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT,
                details TEXT,
                timestamp TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                risk_level TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Create indexes separately
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_session_id ON audit_logs (session_id)")
            .execute(&pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_operator_id ON audit_logs (operator_id)")
            .execute(&pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_timestamp ON audit_logs (timestamp)")
            .execute(&pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_risk_level ON audit_logs (risk_level)")
            .execute(&pool)
            .await?;

        // Create audit_summary table for quick statistics
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_summary (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                operator_id TEXT NOT NULL,
                date TEXT NOT NULL,
                total_actions INTEGER DEFAULT 0,
                high_risk_actions INTEGER DEFAULT 0,
                critical_actions INTEGER DEFAULT 0,
                last_updated TEXT NOT NULL,
                UNIQUE(session_id, operator_id, date)
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn log_action(
        &self,
        session_id: Uuid,
        operator_id: &str,
        action: AuditAction,
        resource: Option<&str>,
        details: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> AuroraResult<i64> {
        let risk_level = self.calculate_risk_level(&action, details);
        let timestamp = Utc::now();

        let result = sqlx::query(
            r#"
            INSERT INTO audit_logs 
            (session_id, operator_id, action, resource, details, timestamp, ip_address, user_agent, risk_level)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session_id.to_string())
        .bind(operator_id)
        .bind(serde_json::to_string(&action)?)
        .bind(resource)
        .bind(details)
        .bind(timestamp.to_rfc3339())
        .bind(ip_address)
        .bind(user_agent)
        .bind(serde_json::to_string(&risk_level)?)
        .execute(&self.pool)
        .await?;

        let log_id = result.last_insert_rowid();

        // Update summary
        self.update_audit_summary(session_id, operator_id, &risk_level, timestamp).await?;

        // Check for compliance violations (but avoid recursion for ComplianceViolation actions)
        if !matches!(action, AuditAction::ComplianceViolation) {
            self.check_compliance_violations(session_id, operator_id, &action, &risk_level).await?;
        }

        Ok(log_id)
    }

    fn calculate_risk_level(&self, action: &AuditAction, details: Option<&str>) -> RiskLevel {
        match action {
            AuditAction::SessionCreated | AuditAction::SessionTerminated => RiskLevel::Low,
            AuditAction::CommandExecuted => {
                if let Some(details) = details {
                    let details_lower = details.to_lowercase();
                    if details_lower.contains("rm -rf") 
                        || details_lower.contains("format") 
                        || details_lower.contains("del /f") {
                        RiskLevel::Critical
                    } else if details_lower.contains("sudo") 
                        || details_lower.contains("su -") 
                        || details_lower.contains("passwd") {
                        RiskLevel::High
                    } else {
                        RiskLevel::Medium
                    }
                } else {
                    RiskLevel::Medium
                }
            }
            AuditAction::FileDeleted | AuditAction::DataExfiltrated => RiskLevel::High,
            AuditAction::PrivilegeEscalated | AuditAction::UnauthorizedAccess 
                | AuditAction::ComplianceViolation => RiskLevel::Critical,
            AuditAction::FileAccessed | AuditAction::NetworkConnection 
                | AuditAction::ProxyUsed => RiskLevel::Medium,
            AuditAction::FileModified => RiskLevel::Medium,
            AuditAction::HeartbeatMissed | AuditAction::AuthenticationFailed => RiskLevel::Medium,
        }
    }

    async fn update_audit_summary(
        &self,
        session_id: Uuid,
        operator_id: &str,
        risk_level: &RiskLevel,
        timestamp: DateTime<Utc>,
    ) -> AuroraResult<()> {
        let date = timestamp.format("%Y-%m-%d").to_string();
        
        let high_risk_increment = if matches!(risk_level, RiskLevel::High) { 1 } else { 0 };
        let critical_increment = if matches!(risk_level, RiskLevel::Critical) { 1 } else { 0 };

        sqlx::query(
            r#"
            INSERT INTO audit_summary 
            (session_id, operator_id, date, total_actions, high_risk_actions, critical_actions, last_updated)
            VALUES (?, ?, ?, 1, ?, ?, ?)
            ON CONFLICT(session_id, operator_id, date) DO UPDATE SET
                total_actions = total_actions + 1,
                high_risk_actions = high_risk_actions + ?,
                critical_actions = critical_actions + ?,
                last_updated = ?
            "#,
        )
        .bind(session_id.to_string())
        .bind(operator_id)
        .bind(date)
        .bind(high_risk_increment)
        .bind(critical_increment)
        .bind(timestamp.to_rfc3339())
        .bind(high_risk_increment)
        .bind(critical_increment)
        .bind(timestamp.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn check_compliance_violations(
        &self,
        session_id: Uuid,
        operator_id: &str,
        action: &AuditAction,
        risk_level: &RiskLevel,
    ) -> AuroraResult<()> {
        // Check for suspicious patterns
        if matches!(risk_level, RiskLevel::Critical) {
            tracing::warn!(
                "Critical risk action detected: session={}, operator={}, action={:?}",
                session_id, operator_id, action
            );

            // Log compliance violation directly to avoid recursion
            self.log_compliance_violation_direct(
                session_id,
                operator_id,
                &format!("Critical risk action: {:?}", action),
            ).await?;
        }

        // Check for rapid succession of high-risk actions
        let recent_high_risk = self.count_recent_high_risk_actions(session_id, operator_id).await?;
        if recent_high_risk > 5 {
            tracing::error!(
                "Multiple high-risk actions detected: session={}, operator={}, count={}",
                session_id, operator_id, recent_high_risk
            );

            // Log compliance violation directly to avoid recursion
            self.log_compliance_violation_direct(
                session_id,
                operator_id,
                &format!("Rapid high-risk actions: {}", recent_high_risk),
            ).await?;
        }

        Ok(())
    }

    // Direct logging method to avoid recursion for compliance violations
    async fn log_compliance_violation_direct(
        &self,
        session_id: Uuid,
        operator_id: &str,
        details: &str,
    ) -> AuroraResult<()> {
        let timestamp = Utc::now();
        let risk_level = RiskLevel::Critical;

        sqlx::query(
            r#"
            INSERT INTO audit_logs 
            (session_id, operator_id, action, resource, details, timestamp, ip_address, user_agent, risk_level)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session_id.to_string())
        .bind(operator_id)
        .bind(serde_json::to_string(&AuditAction::ComplianceViolation)?)
        .bind(None::<String>)
        .bind(details)
        .bind(timestamp.to_rfc3339())
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(serde_json::to_string(&risk_level)?)
        .execute(&self.pool)
        .await?;

        // Update summary
        self.update_audit_summary(session_id, operator_id, &risk_level, timestamp).await?;

        Ok(())
    }

    async fn count_recent_high_risk_actions(
        &self,
        session_id: Uuid,
        operator_id: &str,
    ) -> AuroraResult<i64> {
        let five_minutes_ago = Utc::now() - chrono::Duration::minutes(5);

        let result = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM audit_logs 
            WHERE session_id = ? AND operator_id = ? 
            AND timestamp > ? 
            AND (risk_level = ? OR risk_level = ?)
            "#,
        )
        .bind(session_id.to_string())
        .bind(operator_id)
        .bind(five_minutes_ago.to_rfc3339())
        .bind(serde_json::to_string(&RiskLevel::High)?)
        .bind(serde_json::to_string(&RiskLevel::Critical)?)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get::<i64, _>("count"))
    }

    pub async fn get_session_audit_logs(
        &self,
        session_id: &Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AuroraResult<Vec<AuditLog>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);

        let rows = sqlx::query(
            r#"
            SELECT * FROM audit_logs 
            WHERE session_id = ? 
            ORDER BY timestamp DESC 
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(session_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_audit_logs(rows).await
    }

    pub async fn get_operator_audit_logs(
        &self,
        operator_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> AuroraResult<Vec<AuditLog>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);

        let rows = sqlx::query(
            r#"
            SELECT * FROM audit_logs 
            WHERE operator_id = ? 
            ORDER BY timestamp DESC 
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(operator_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_audit_logs(rows).await
    }

    pub async fn get_high_risk_logs(
        &self,
        hours: Option<i64>,
        limit: Option<i64>,
    ) -> AuroraResult<Vec<AuditLog>> {
        let hours = hours.unwrap_or(24);
        let limit = limit.unwrap_or(100);
        let since = Utc::now() - chrono::Duration::hours(hours);

        let rows = sqlx::query(
            r#"
            SELECT * FROM audit_logs 
            WHERE timestamp > ? 
            AND (risk_level = ? OR risk_level = ?)
            ORDER BY timestamp DESC 
            LIMIT ?
            "#,
        )
        .bind(since.to_rfc3339())
        .bind(serde_json::to_string(&RiskLevel::High)?)
        .bind(serde_json::to_string(&RiskLevel::Critical)?)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        self.rows_to_audit_logs(rows).await
    }

    pub async fn get_audit_summary(
        &self,
        session_id: Option<Uuid>,
        operator_id: Option<&str>,
        days: Option<i64>,
    ) -> AuroraResult<Vec<AuditSummary>> {
        let days = days.unwrap_or(7);
        let since_date = (Utc::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d").to_string();

        let mut query = "SELECT * FROM audit_summary WHERE date >= ?".to_string();
        let mut params: Vec<String> = vec![since_date];

        if let Some(sid) = session_id {
            query.push_str(" AND session_id = ?");
            params.push(sid.to_string());
        }

        if let Some(oid) = operator_id {
            query.push_str(" AND operator_id = ?");
            params.push(oid.to_string());
        }

        query.push_str(" ORDER BY date DESC");

        let mut sql_query = sqlx::query(&query);
        for param in params {
            sql_query = sql_query.bind(param);
        }

        let rows = sql_query.fetch_all(&self.pool).await?;

        let mut summaries = Vec::new();
        for row in rows {
            let summary = AuditSummary {
                session_id: Uuid::parse_str(&row.get::<String, _>("session_id"))
                    .map_err(|e| AuroraError::Generic(e.into()))?,
                operator_id: row.get("operator_id"),
                date: row.get("date"),
                total_actions: row.get("total_actions"),
                high_risk_actions: row.get("high_risk_actions"),
                critical_actions: row.get("critical_actions"),
                last_updated: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_updated"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
            };
            summaries.push(summary);
        }

        Ok(summaries)
    }

    async fn rows_to_audit_logs(&self, rows: Vec<sqlx::sqlite::SqliteRow>) -> AuroraResult<Vec<AuditLog>> {
        let mut logs = Vec::new();
        for row in rows {
            let log = AuditLog {
                id: row.get("id"),
                session_id: Uuid::parse_str(&row.get::<String, _>("session_id"))
                    .map_err(|e| AuroraError::Generic(e.into()))?,
                operator_id: row.get("operator_id"),
                action: serde_json::from_str(&row.get::<String, _>("action"))?,
                resource: row.get("resource"),
                details: row.get("details"),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))
                    .map_err(|e| AuroraError::Generic(e.into()))?
                    .with_timezone(&Utc),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                risk_level: serde_json::from_str(&row.get::<String, _>("risk_level"))?,
            };
            logs.push(log);
        }
        Ok(logs)
    }

    pub async fn cleanup_old_logs(&self, days_to_keep: i64) -> AuroraResult<i64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days_to_keep);

        let result = sqlx::query(
            "DELETE FROM audit_logs WHERE timestamp < ?"
        )
        .bind(cutoff_date.to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Also cleanup old summaries
        let cutoff_date_str = cutoff_date.format("%Y-%m-%d").to_string();
        sqlx::query(
            "DELETE FROM audit_summary WHERE date < ?"
        )
        .bind(cutoff_date_str)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub session_id: Uuid,
    pub operator_id: String,
    pub date: String,
    pub total_actions: i64,
    pub high_risk_actions: i64,
    pub critical_actions: i64,
    pub last_updated: DateTime<Utc>,
}