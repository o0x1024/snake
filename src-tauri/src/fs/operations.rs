use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{AuroraResult, FileSystemError};

pub struct FileOperations;

impl FileOperations {
    pub fn new() -> Self {
        Self
    }

    pub async fn read_file<P: AsRef<Path>>(&self, path: P) -> AuroraResult<Vec<u8>> {
        let mut file = fs::File::open(path).await
            .map_err(|_| FileSystemError::FileNotFound("File not found".to_string()))?;
        
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await
            .map_err(|_| FileSystemError::PermissionDenied)?;
        
        Ok(contents)
    }

    pub async fn write_file<P: AsRef<Path>>(&self, path: P, data: &[u8]) -> AuroraResult<()> {
        let mut file = fs::File::create(path).await
            .map_err(|_| FileSystemError::PermissionDenied)?;
        
        file.write_all(data).await
            .map_err(|_| FileSystemError::PermissionDenied)?;
        
        file.flush().await
            .map_err(|_| FileSystemError::PermissionDenied)?;
        
        Ok(())
    }

    pub async fn list_directory<P: AsRef<Path>>(&self, path: P) -> AuroraResult<Vec<String>> {
        let mut entries = fs::read_dir(path).await
            .map_err(|_| FileSystemError::FileNotFound("Directory not found".to_string()))?;
        
        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await
            .map_err(|_| FileSystemError::PermissionDenied)? {
            
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }
        
        Ok(files)
    }

    pub async fn delete_file<P: AsRef<Path>>(&self, path: P) -> AuroraResult<()> {
        fs::remove_file(path).await
            .map_err(|_| FileSystemError::PermissionDenied)?;
        Ok(())
    }

    pub async fn create_directory<P: AsRef<Path>>(&self, path: P) -> AuroraResult<()> {
        fs::create_dir_all(path).await
            .map_err(|_| FileSystemError::PermissionDenied)?;
        Ok(())
    }
}