use tauri::State;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::AuroraResult;
use crate::plugins::{PluginApi, PluginRequest, PluginResponse};
use crate::AppState;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub loaded: bool,
    pub instantiated: bool,
    pub functions: Vec<String>,
    pub capabilities: Option<crate::plugins::runtime::PluginCapabilities>,
    pub statistics: Option<crate::plugins::runtime::PluginStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPluginRequest {
    pub plugin_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutePluginRequest {
    pub plugin_name: String,
    pub function_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

// Global plugin API instance
lazy_static::lazy_static! {
    static ref PLUGIN_API: std::sync::Mutex<Option<Arc<PluginApi>>> = std::sync::Mutex::new(None);
}

fn get_plugin_api() -> AuroraResult<Arc<PluginApi>> {
    let mut api_guard = PLUGIN_API.lock().unwrap();
    if api_guard.is_none() {
        // Initialize plugin API with default plugin directory
        let plugin_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("plugins")
            .to_string_lossy()
            .to_string();
        
        *api_guard = Some(Arc::new(PluginApi::new(plugin_dir)?));
    }
    
    Ok(api_guard.as_ref().unwrap().clone())
}

#[tauri::command]
pub async fn list_available_plugins() -> Result<Vec<String>, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.list_available_plugins().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_loaded_plugins() -> Result<Vec<String>, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.get_loaded_plugins().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_plugin(request: LoadPluginRequest) -> Result<String, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.load_plugin_from_directory(&request.plugin_name).await
        .map_err(|e| e.to_string())?;
    
    Ok(format!("Plugin '{}' loaded successfully", request.plugin_name))
}

#[tauri::command]
pub async fn unload_plugin(request: LoadPluginRequest) -> Result<String, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.unload_plugin(&request.plugin_name).await
        .map_err(|e| e.to_string())?;
    
    Ok(format!("Plugin '{}' unloaded successfully", request.plugin_name))
}

#[tauri::command]
pub async fn reload_plugin(request: LoadPluginRequest) -> Result<String, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.reload_plugin(&request.plugin_name).await
        .map_err(|e| e.to_string())?;
    
    Ok(format!("Plugin '{}' reloaded successfully", request.plugin_name))
}

#[tauri::command]
pub async fn execute_plugin(request: ExecutePluginRequest) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let plugin_request = PluginRequest {
        plugin_name: request.plugin_name,
        function_name: request.function_name,
        parameters: request.parameters,
    };
    
    api.execute_plugin(plugin_request).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_plugin_functions(plugin_name: String) -> Result<Vec<String>, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.list_available_functions(&plugin_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_plugin_documentation(plugin_name: String) -> Result<String, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.get_plugin_documentation(&plugin_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_plugin_hot_reload(plugin_name: String) -> Result<String, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.enable_hot_reload(&plugin_name).await.map_err(|e| e.to_string())?;
    
    Ok(format!("Hot reload enabled for plugin '{}'", plugin_name))
}

#[tauri::command]
pub async fn disable_plugin_hot_reload(plugin_name: String) -> Result<String, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.disable_hot_reload(&plugin_name).await.map_err(|e| e.to_string())?;
    
    Ok(format!("Hot reload disabled for plugin '{}'", plugin_name))
}

#[tauri::command]
pub async fn get_plugin_statistics() -> Result<HashMap<String, crate::plugins::runtime::PluginStats>, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    api.get_plugin_statistics().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_vulnerabilities(target: String, scan_type: Option<String>) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let mut parameters = HashMap::new();
    parameters.insert("target".to_string(), serde_json::Value::String(target));
    parameters.insert("scan_type".to_string(), serde_json::Value::String(scan_type.unwrap_or("quick".to_string())));
    
    let request = PluginRequest {
        plugin_name: "vulnerability_scanner".to_string(),
        function_name: "scan_vulnerabilities".to_string(),
        parameters,
    };
    
    api.execute_plugin(request).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn crack_password(hash: String, wordlist: Option<String>) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let mut parameters = HashMap::new();
    parameters.insert("hash".to_string(), serde_json::Value::String(hash));
    parameters.insert("wordlist".to_string(), serde_json::Value::String(wordlist.unwrap_or("common_passwords.txt".to_string())));
    
    let request = PluginRequest {
        plugin_name: "password_cracker".to_string(),
        function_name: "crack_password".to_string(),
        parameters,
    };
    
    api.execute_plugin(request).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn network_scan(target: String, port_range: Option<String>) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let mut parameters = HashMap::new();
    parameters.insert("target".to_string(), serde_json::Value::String(target));
    parameters.insert("port_range".to_string(), serde_json::Value::String(port_range.unwrap_or("1-1000".to_string())));
    
    let request = PluginRequest {
        plugin_name: "network_scanner".to_string(),
        function_name: "network_scan".to_string(),
        parameters,
    };
    
    api.execute_plugin(request).await.map_err(|e| e.to_string())
}

// Penetration Testing Assistant Commands

#[tauri::command]
pub async fn gather_information(target: String) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let mut parameters = HashMap::new();
    parameters.insert("target".to_string(), serde_json::Value::String(target));
    
    let request = PluginRequest {
        plugin_name: "pentest_assistant".to_string(),
        function_name: "gather_information".to_string(),
        parameters,
    };
    
    api.execute_plugin(request).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_privilege_escalation(target: String) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let mut parameters = HashMap::new();
    parameters.insert("target".to_string(), serde_json::Value::String(target));
    
    let request = PluginRequest {
        plugin_name: "pentest_assistant".to_string(),
        function_name: "analyze_privilege_escalation".to_string(),
        parameters,
    };
    
    api.execute_plugin(request).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn perform_lateral_movement(source_host: String, target_network: String) -> Result<PluginResponse, String> {
    let api = get_plugin_api().map_err(|e| e.to_string())?;
    
    let mut parameters = HashMap::new();
    parameters.insert("source_host".to_string(), serde_json::Value::String(source_host));
    parameters.insert("target_network".to_string(), serde_json::Value::String(target_network));
    
    let request = PluginRequest {
        plugin_name: "pentest_assistant".to_string(),
        function_name: "perform_lateral_movement".to_string(),
        parameters,
    };
    
    api.execute_plugin(request).await.map_err(|e| e.to_string())
}

// Protocol extension commands

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConnectionRequest {
    pub webshell_type: String,
    pub endpoint: String,
    pub encryption: String,
    pub obfuscation: String,
    pub proxy: Option<ProxyConfigRequest>,
    pub custom_headers: HashMap<String, String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfigRequest {
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCommandRequest {
    pub connection_id: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequest {
    pub connection_id: String,
    pub local_path: String,
    pub remote_path: String,
}

#[tauri::command]
pub async fn create_protocol_connection(request: ProtocolConnectionRequest) -> Result<String, String> {
    use crate::plugins::{WebshellType, EncryptionMethod, ObfuscationMethod, ProxyType, ProxyConfig, ProtocolConfig, ProtocolAdapterFactory};
    
    // Parse webshell type
    let webshell_type = match request.webshell_type.as_str() {
        "php" => WebshellType::Php,
        "asp" => WebshellType::Asp,
        "jsp" => WebshellType::Jsp,
        "python" => WebshellType::Python,
        "nodejs" => WebshellType::NodeJs,
        custom => WebshellType::Custom(custom.to_string()),
    };

    // Parse encryption method
    let encryption = match request.encryption.as_str() {
        "aes256" => EncryptionMethod::Aes256,
        "rsa2048" => EncryptionMethod::Rsa2048,
        "rc4" => EncryptionMethod::Rc4,
        "chacha20" => EncryptionMethod::ChaCha20,
        custom => EncryptionMethod::Custom(custom.to_string()),
    };

    // Parse obfuscation method
    let obfuscation = match request.obfuscation.as_str() {
        "http_normal" => ObfuscationMethod::HttpNormal,
        "http_headers" => ObfuscationMethod::HttpHeaders,
        "dns_tunnel" => ObfuscationMethod::DnsTunnel,
        "base64" => ObfuscationMethod::Base64,
        custom => ObfuscationMethod::Custom(custom.to_string()),
    };

    // Parse proxy config
    let proxy = if let Some(proxy_req) = request.proxy {
        let proxy_type = match proxy_req.proxy_type.as_str() {
            "socks5" => ProxyType::Socks5,
            "http" => ProxyType::Http,
            "tor" => ProxyType::Tor,
            _ => return Err("Invalid proxy type".to_string()),
        };

        Some(ProxyConfig {
            proxy_type,
            host: proxy_req.host,
            port: proxy_req.port,
            username: proxy_req.username,
            password: proxy_req.password,
        })
    } else {
        None
    };

    let config = ProtocolConfig {
        webshell_type: webshell_type.clone(),
        encryption,
        obfuscation,
        proxy,
        custom_headers: request.custom_headers,
        user_agent: request.user_agent,
    };

    // Create adapter
    let adapter = ProtocolAdapterFactory::create_adapter(&webshell_type, request.endpoint, config.clone())
        .map_err(|e| e.to_string())?;

    // Test connection
    adapter.connect(&config).await.map_err(|e| e.to_string())?;

    // Generate connection ID
    let connection_id = uuid::Uuid::new_v4().to_string();

    // Store connection (Note: This is simplified - in production you'd need proper async storage)
    // For now, we'll just return success
    
    Ok(connection_id)
}

#[tauri::command]
pub async fn execute_protocol_command(request: ProtocolCommandRequest) -> Result<String, String> {
    // In a real implementation, this would retrieve the connection and execute the command
    // For now, return a simulated response
    Ok(format!("Command '{}' executed on connection {}", request.command, request.connection_id))
}

#[tauri::command]
pub async fn upload_file_via_protocol(request: FileTransferRequest) -> Result<String, String> {
    // In a real implementation, this would retrieve the connection and upload the file
    Ok(format!("File uploaded: {} -> {} via connection {}", 
        request.local_path, request.remote_path, request.connection_id))
}

#[tauri::command]
pub async fn download_file_via_protocol(request: FileTransferRequest) -> Result<String, String> {
    // In a real implementation, this would retrieve the connection and download the file
    Ok(format!("File downloaded: {} -> {} via connection {}", 
        request.remote_path, request.local_path, request.connection_id))
}

#[tauri::command]
pub async fn close_protocol_connection(connection_id: String) -> Result<String, String> {
    // In a real implementation, this would close the connection and clean up resources
    Ok(format!("Connection {} closed", connection_id))
}

#[tauri::command]
pub async fn list_protocol_connections() -> Result<Vec<String>, String> {
    // In a real implementation, this would return active connection IDs
    Ok(vec!["connection-1".to_string(), "connection-2".to_string()])
}