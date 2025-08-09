use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverConfig {
    pub endpoint: String,
    pub password: String,
    pub charset: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsEntry {
    pub name: String,
    pub path: String,
    pub r#type: String,
    pub size: u64,
    pub perm: String,
    pub mtime: String,
    pub hidden: bool,
}

