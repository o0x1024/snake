use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

use crate::error::{AuroraResult, FileSystemError};

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub data: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub access_count: u64,
}

pub struct FileCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_size: usize,
    max_entries: usize,
}

impl FileCache {
    pub fn new(max_size: usize, max_entries: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_entries,
        }
    }

    pub async fn get(&self, key: &str) -> AuroraResult<Option<Vec<u8>>> {
        let mut cache = self.cache.write().await;
        
        if let Some(entry) = cache.get_mut(key) {
            entry.access_count += 1;
            Ok(Some(entry.data.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn put(&self, key: String, data: Vec<u8>) -> AuroraResult<()> {
        let mut cache = self.cache.write().await;
        
        // Check size limits
        if data.len() > self.max_size {
            return Err(FileSystemError::CacheError.into());
        }

        // Evict if necessary
        if cache.len() >= self.max_entries {
            self.evict_lru(&mut cache).await?;
        }

        let entry = CacheEntry {
            data,
            timestamp: Utc::now(),
            access_count: 1,
        };

        cache.insert(key, entry);
        Ok(())
    }

    pub async fn remove(&self, key: &str) -> AuroraResult<()> {
        let mut cache = self.cache.write().await;
        cache.remove(key);
        Ok(())
    }

    pub async fn clear(&self) -> AuroraResult<()> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }

    async fn evict_lru(&self, cache: &mut HashMap<String, CacheEntry>) -> AuroraResult<()> {
        if cache.is_empty() {
            return Ok(());
        }

        // Find the least recently used entry (lowest access count and oldest timestamp)
        let lru_key = cache.iter()
            .min_by(|(_, a), (_, b)| {
                a.access_count.cmp(&b.access_count)
                    .then(a.timestamp.cmp(&b.timestamp))
            })
            .map(|(k, _)| k.clone());

        if let Some(key) = lru_key {
            cache.remove(&key);
        }

        Ok(())
    }

    pub async fn stats(&self) -> AuroraResult<CacheStats> {
        let cache = self.cache.read().await;
        
        let total_entries = cache.len();
        let total_size: usize = cache.values().map(|entry| entry.data.len()).sum();
        let total_accesses: u64 = cache.values().map(|entry| entry.access_count).sum();

        Ok(CacheStats {
            total_entries,
            total_size,
            total_accesses,
            max_entries: self.max_entries,
            max_size: self.max_size,
        })
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size: usize,
    pub total_accesses: u64,
    pub max_entries: usize,
    pub max_size: usize,
}