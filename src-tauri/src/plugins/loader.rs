use std::path::Path;
use tokio::fs;
use serde::{Deserialize, Serialize};

use crate::error::{AuroraResult, PluginError};
use super::runtime::{PluginRuntime, PluginCapabilities};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entry_point: String,
    pub permissions: Vec<String>,
    pub dependencies: Vec<String>,
    pub capabilities: Option<PluginCapabilities>,
    pub hot_reload: Option<bool>,
}

pub struct PluginLoader {
    runtime: PluginRuntime,
    plugin_directory: String,
}

impl PluginLoader {
    pub fn new(plugin_directory: String) -> AuroraResult<Self> {
        let runtime = PluginRuntime::new()?;
        
        Ok(Self {
            runtime,
            plugin_directory,
        })
    }

    pub async fn load_plugin_from_directory(&self, plugin_name: &str) -> AuroraResult<()> {
        let plugin_path = Path::new(&self.plugin_directory).join(plugin_name);
        
        if !plugin_path.exists() {
            return Err(PluginError::NotFound(plugin_name.to_string()).into());
        }

        // Load manifest
        let manifest_path = plugin_path.join("manifest.json");
        let manifest_content = fs::read_to_string(manifest_path).await
            .map_err(|_| PluginError::LoadFailed("Failed to read manifest".to_string()))?;
        
        let manifest: PluginManifest = serde_json::from_str(&manifest_content)
            .map_err(|_| PluginError::LoadFailed("Invalid manifest format".to_string()))?;

        // Validate permissions
        self.validate_permissions(&manifest.permissions)?;

        // Load WASM binary
        let wasm_path = plugin_path.join(&manifest.entry_point);
        let wasm_bytes = fs::read(&wasm_path).await
            .map_err(|_| PluginError::LoadFailed("Failed to read WASM binary".to_string()))?;

        // Load into runtime
        self.runtime.load_plugin(manifest.name.clone(), &wasm_bytes).await?;
        self.runtime.instantiate_plugin(&manifest.name).await?;

        // Set plugin capabilities if specified
        if let Some(capabilities) = manifest.capabilities {
            self.runtime.set_plugin_capabilities(&manifest.name, capabilities).await?;
        }

        // Enable hot reload if requested
        if manifest.hot_reload.unwrap_or(false) {
            self.runtime.enable_hot_reload(&manifest.name, wasm_path).await?;
        }

        tracing::info!("Successfully loaded plugin: {}", manifest.name);
        Ok(())
    }

    pub async fn load_plugin_from_bytes(
        &self,
        name: String,
        wasm_bytes: &[u8],
        manifest: PluginManifest,
    ) -> AuroraResult<()> {
        // Validate permissions
        self.validate_permissions(&manifest.permissions)?;

        // Load into runtime
        self.runtime.load_plugin(name.clone(), wasm_bytes).await?;
        self.runtime.instantiate_plugin(&name).await?;

        // Set plugin capabilities if specified
        if let Some(capabilities) = manifest.capabilities {
            self.runtime.set_plugin_capabilities(&name, capabilities).await?;
        }

        tracing::info!("Successfully loaded plugin from bytes: {}", name);
        Ok(())
    }

    pub async fn unload_plugin(&self, name: &str) -> AuroraResult<()> {
        self.runtime.unload_plugin(name).await?;
        tracing::info!("Successfully unloaded plugin: {}", name);
        Ok(())
    }

    pub async fn reload_plugin(&self, name: &str) -> AuroraResult<()> {
        self.unload_plugin(name).await?;
        self.load_plugin_from_directory(name).await?;
        Ok(())
    }

    pub async fn list_available_plugins(&self) -> AuroraResult<Vec<String>> {
        let plugin_dir = Path::new(&self.plugin_directory);
        
        if !plugin_dir.exists() {
            return Ok(vec![]);
        }

        let mut plugins = Vec::new();
        let mut entries = fs::read_dir(plugin_dir).await
            .map_err(|_| PluginError::LoadFailed("Failed to read plugin directory".to_string()))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|_| PluginError::LoadFailed("Failed to read directory entry".to_string()))? {
            
            if entry.file_type().await
                .map_err(|_| PluginError::LoadFailed("Failed to get file type".to_string()))?
                .is_dir() {
                
                if let Some(name) = entry.file_name().to_str() {
                    plugins.push(name.to_string());
                }
            }
        }

        Ok(plugins)
    }

    pub async fn get_plugin_manifest(&self, plugin_name: &str) -> AuroraResult<PluginManifest> {
        let manifest_path = Path::new(&self.plugin_directory)
            .join(plugin_name)
            .join("manifest.json");

        let manifest_content = fs::read_to_string(manifest_path).await
            .map_err(|_| PluginError::NotFound(format!("Manifest for plugin '{}'", plugin_name)))?;

        let manifest: PluginManifest = serde_json::from_str(&manifest_content)
            .map_err(|_| PluginError::LoadFailed("Invalid manifest format".to_string()))?;

        Ok(manifest)
    }

    fn validate_permissions(&self, permissions: &[String]) -> AuroraResult<()> {
        let allowed_permissions = vec![
            "network.http".to_string(),
            "filesystem.read".to_string(),
            "filesystem.write".to_string(),
            "crypto.encrypt".to_string(),
            "crypto.decrypt".to_string(),
            "system.execute".to_string(),
        ];

        for permission in permissions {
            if !allowed_permissions.contains(permission) {
                return Err(PluginError::LoadFailed(
                    format!("Permission '{}' is not allowed", permission)
                ).into());
            }
        }

        Ok(())
    }

    pub async fn execute_plugin_function(
        &self,
        plugin_name: &str,
        function_name: &str,
        args: &[serde_json::Value],
    ) -> AuroraResult<Vec<serde_json::Value>> {
        self.runtime.execute_plugin_function(plugin_name, function_name, args).await
    }

    pub async fn enable_hot_reload(&self, plugin_name: &str) -> AuroraResult<()> {
        let plugin_path = Path::new(&self.plugin_directory)
            .join(plugin_name)
            .join("plugin.wasm");
        
        self.runtime.enable_hot_reload(plugin_name, plugin_path).await
    }

    pub async fn disable_hot_reload(&self, plugin_name: &str) -> AuroraResult<()> {
        self.runtime.disable_hot_reload(plugin_name).await
    }

    pub async fn get_plugin_statistics(&self) -> AuroraResult<std::collections::HashMap<String, super::runtime::PluginStats>> {
        self.runtime.get_plugin_statistics().await
    }

    pub async fn get_loaded_plugins(&self) -> AuroraResult<Vec<String>> {
        self.runtime.list_loaded_plugins().await
    }
}