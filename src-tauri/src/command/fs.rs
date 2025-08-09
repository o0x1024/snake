use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub r#type: String,
    pub size: u64,
    pub permissions: String,
    pub modified: String,
    pub owner: String,
    pub is_hidden: bool,
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

#[tauri::command]
pub async fn list_directory(
    _session_id: String,
    path: String,
    show_hidden: bool,
) -> Result<Vec<FileItem>, String> {
    let base = if path.is_empty() { "." } else { &path };
    let pathbuf = PathBuf::from(base);
    let entries = fs::read_dir(&pathbuf).map_err(|e| format!("Failed to read dir: {}", e))?;

    let mut items: Vec<FileItem> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let meta = entry.metadata().map_err(|e| format!("Failed to read metadata: {}", e))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let is_hidden_flag = is_hidden(&file_name);
        if is_hidden_flag && !show_hidden { continue; }

        let file_type = if meta.is_dir() { "directory" } else { "file" }.to_string();
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339().into())
            .unwrap_or_else(|| "".into());

        let item = FileItem {
            name: file_name.clone(),
            path: entry.path().to_string_lossy().to_string(),
            r#type: file_type,
            size: meta.len(),
            permissions: format!("{:?}", meta.permissions()),
            modified,
            owner: String::from("local"),
            is_hidden: is_hidden_flag,
        };
        items.push(item);
    }

    // Sort directories first then files by name
    items.sort_by(|a, b| {
        if a.r#type != b.r#type { return if a.r#type == "directory" { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }; }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    Ok(items)
}

#[tauri::command]
pub async fn download_file(
    session_id: String,
    remote_path: String,
) -> Result<Vec<u8>, String> {
    use crate::command::driver::ws_execute;
    use crate::AppState;
    use tauri::State;
    
    // This is a simplified implementation - in a real scenario you'd need the AppState
    // For now, return an error indicating the functionality needs a webshell endpoint
    Err("Download requires webshell integration - use download_file_with_endpoint instead".to_string())
}

#[tauri::command]
pub async fn download_file_with_endpoint(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: String,
    remote_path: String,
) -> Result<Vec<u8>, String> {
    use crate::command::driver::ws_execute;
    
    // Use base64 encoding to safely transfer binary files
    let download_cmd = format!("base64 -w 0 {}", shell_escape::escape(remote_path.into()));
    let result = ws_execute(state, session_id, endpoint, download_cmd).await?;
    
    // Decode base64 content
    use base64::{Engine as _, engine::general_purpose};
    let file_data = general_purpose::STANDARD
        .decode(result.stdout.trim())
        .map_err(|e| format!("Failed to decode file content: {}", e))?;
    
    Ok(file_data)
}

#[tauri::command]
pub async fn read_file(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    file_path: String,
) -> Result<String, String> {
    if let Some(ep) = endpoint {
        // Remote file reading via shell
        let read_cmd = format!("cat {}", shell_escape::escape(file_path.into()));
        let result = crate::command::driver::ws_execute(state, session_id, ep, read_cmd).await?;
        Ok(result.stdout)
    } else {
        // Local fallback
        std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read file {}: {}", file_path, e))
    }
}

#[tauri::command]
pub async fn write_file(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    file_path: String,
    content: String,
) -> Result<(), String> {
    if let Some(ep) = endpoint {
        // Remote file writing via shell - use base64 to handle special characters
        use base64::{Engine as _, engine::general_purpose};
        let b64_content = general_purpose::STANDARD.encode(content.as_bytes());
        let write_cmd = format!(
            "echo {} | base64 -d > {}",
            shell_escape::escape(b64_content.into()),
            shell_escape::escape(file_path.clone().into())
        );
        let _ = crate::command::driver::ws_execute(state, session_id, ep, write_cmd).await?;
        Ok(())
    } else {
        // Local fallback
        std::fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write file {}: {}", file_path, e))
    }
}

#[tauri::command]
pub async fn rename_file(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    if let Some(ep) = endpoint {
        // Remote file renaming via shell
        let rename_cmd = format!(
            "mv {} {}",
            shell_escape::escape(old_path.into()),
            shell_escape::escape(new_path.into())
        );
        let _ = crate::command::driver::ws_execute(state, session_id, ep, rename_cmd).await?;
        Ok(())
    } else {
        // Local fallback
        std::fs::rename(&old_path, &new_path)
            .map_err(|e| format!("Failed to rename {} to {}: {}", old_path, new_path, e))
    }
}

#[tauri::command]
pub async fn create_directory(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    dir_path: String,
) -> Result<(), String> {
    if let Some(ep) = endpoint {
        // Remote directory creation via shell
        let mkdir_cmd = format!("mkdir -p {}", shell_escape::escape(dir_path.into()));
        let _ = crate::command::driver::ws_execute(state, session_id, ep, mkdir_cmd).await?;
        Ok(())
    } else {
        // Local fallback
        std::fs::create_dir_all(&dir_path)
            .map_err(|e| format!("Failed to create directory {}: {}", dir_path, e))
    }
}

#[tauri::command]
pub async fn copy_file(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    source_path: String,
    dest_path: String,
) -> Result<(), String> {
    if let Some(ep) = endpoint {
        // Remote file copying via shell
        let copy_cmd = format!(
            "cp -r {} {}",
            shell_escape::escape(source_path.into()),
            shell_escape::escape(dest_path.into())
        );
        let _ = crate::command::driver::ws_execute(state, session_id, ep, copy_cmd).await?;
        Ok(())
    } else {
        // Local fallback
        let source = std::path::Path::new(&source_path);
        let dest = std::path::Path::new(&dest_path);
        
        if source.is_dir() {
            copy_dir_all(source, dest)
                .map_err(|e| format!("Failed to copy directory {} to {}: {}", source_path, dest_path, e))
        } else {
            std::fs::copy(source, dest)
                .map(|_| ())
                .map_err(|e| format!("Failed to copy file {} to {}: {}", source_path, dest_path, e))
        }
    }
}

// Helper function for recursive directory copying
fn copy_dir_all(src: impl AsRef<std::path::Path>, dst: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_file_info(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    file_path: String,
) -> Result<FileItem, String> {
    if let Some(ep) = endpoint {
        // Remote file info via shell
        let stat_cmd = format!("stat -c '%n|%s|%Y|%A' {}", shell_escape::escape(file_path.clone().into()));
        let result = crate::command::driver::ws_execute(state, session_id, ep, stat_cmd).await?;
        
        let parts: Vec<&str> = result.stdout.trim().split('|').collect();
        if parts.len() >= 4 {
            let name = std::path::Path::new(&file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size = parts[1].parse::<u64>().unwrap_or(0);
            let modified = chrono::DateTime::from_timestamp(parts[2].parse::<i64>().unwrap_or(0), 0)
                .unwrap_or_default()
                .to_rfc3339();
            let permissions = parts[3].to_string();
            let is_dir = permissions.starts_with('d');
            
            Ok(FileItem {
                name: name.clone(),
                path: file_path,
                r#type: if is_dir { "directory" } else { "file" }.to_string(),
                size,
                permissions,
                modified,
                owner: "remote".to_string(),
                is_hidden: name.starts_with('.'),
            })
        } else {
            Err("Failed to parse file info".to_string())
        }
    } else {
        // Local fallback
        let path = std::path::Path::new(&file_path);
        let metadata = path.metadata()
            .map_err(|e| format!("Failed to get file info for {}: {}", file_path, e))?;
        
        let name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339().into())
            .unwrap_or_else(|| "".into());
        
        Ok(FileItem {
            name: name.clone(),
            path: file_path,
            r#type: if metadata.is_dir() { "directory" } else { "file" }.to_string(),
            size: metadata.len(),
            permissions: format!("{:?}", metadata.permissions()),
            modified,
            owner: "local".to_string(),
            is_hidden: name.starts_with('.'),
        })
    }
}

#[tauri::command]
pub async fn delete_files(state: State<'_, crate::AppState>, session_id: String, endpoint: Option<String>, paths: Vec<String>) -> Result<(), String> {
    if let Some(ep) = endpoint {
        // Remote deletion via shell
        let joined = paths
            .into_iter()
            .map(|p| shell_escape::escape(p.into()).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let cmd = format!("rm -rf {}", joined);
        let _ = crate::command::driver::ws_execute(state, session_id, ep, cmd).await?;
        Ok(())
    } else {
        // Local fallback
        for p in paths {
            let pb = std::path::PathBuf::from(&p);
            if pb.is_dir() {
                std::fs::remove_dir_all(&pb).map_err(|e| format!("Failed to remove dir {}: {}", p, e))?;
            } else {
                std::fs::remove_file(&pb).map_err(|e| format!("Failed to remove file {}: {}", p, e))?;
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn upload_file(
    state: State<'_, crate::AppState>,
    session_id: String,
    endpoint: Option<String>,
    file_name: String,
    remote_path: String,
    file_data: Vec<u8>,
) -> Result<(), String> {
    if let Some(ep) = endpoint {
        // Encode to base64 and recreate remotely
        use base64::{Engine as _, engine::general_purpose};
        let b64 = general_purpose::STANDARD.encode(&file_data);
        let remote_full = if remote_path.ends_with('/') { format!("{}{}", remote_path, file_name) } else { format!("{}/{}", remote_path, file_name) };
        let cmd = format!(
            "echo {} | base64 -d > {}",
            shell_escape::escape(b64.into()),
            shell_escape::escape(remote_full.into()),
        );
        let _ = crate::command::driver::ws_execute(state, session_id, ep, cmd).await?;
        Ok(())
    } else {
        // Local fallback
        let remote_full = if remote_path.ends_with('/') { format!("{}{}", remote_path, file_name) } else { format!("{}/{}", remote_path, file_name) };
        std::fs::write(&remote_full, &file_data).map_err(|e| format!("Failed to write local file: {}", e))?;
        Ok(())
    }
}

