use anyhow::Context;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::fs;

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub size: u64,
    pub last_access: u128,
}

pub struct TransformCache {
    dir: PathBuf,
    max_bytes: u64,
    entries: Mutex<HashMap<String, CacheEntry>>,
}

impl TransformCache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> Self {
        Self {
            dir,
            max_bytes,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn key(image_id: &str, query: &str) -> String {
        let mut hash = Sha256::new();
        hash.update(image_id.as_bytes());
        hash.update(b":");
        hash.update(query.as_bytes());
        hex::encode(hash.finalize())
    }

    pub fn path_for(&self, key: &str, ext: &str) -> PathBuf {
        self.dir.join(format!("{key}.{ext}"))
    }

    pub async fn get(&self, key: &str) -> Option<PathBuf> {
        let path = {
            let mut entries = self.entries.lock().ok()?;
            let entry = entries.get_mut(key)?;
            entry.last_access = now_ms();
            entry.path.clone()
        };
        if fs::metadata(&path).await.is_ok() {
            Some(path)
        } else {
            let _ = self.entries.lock().map(|mut entries| entries.remove(key));
            None
        }
    }

    pub async fn insert(&self, key: String, path: PathBuf, size: u64) -> anyhow::Result<()> {
        {
            let mut entries = self.entries.lock().expect("cache lock poisoned");
            entries.insert(
                key,
                CacheEntry {
                    path,
                    size,
                    last_access: now_ms(),
                },
            );
        }
        self.evict().await
    }

    pub async fn clear(&self) -> anyhow::Result<()> {
        let paths = {
            let mut entries = self.entries.lock().expect("cache lock poisoned");
            let paths = entries.values().map(|e| e.path.clone()).collect::<Vec<_>>();
            entries.clear();
            paths
        };
        for path in paths {
            let _ = fs::remove_file(path).await;
        }
        Ok(())
    }

    pub fn stats(&self) -> serde_json::Value {
        let entries = self.entries.lock().expect("cache lock poisoned");
        let bytes: u64 = entries.values().map(|e| e.size).sum();
        serde_json::json!({
            "items": entries.len(),
            "bytes": bytes,
            "max_bytes": self.max_bytes
        })
    }

    async fn evict(&self) -> anyhow::Result<()> {
        loop {
            let victim = {
                let entries = self.entries.lock().expect("cache lock poisoned");
                let total: u64 = entries.values().map(|e| e.size).sum();
                if total <= self.max_bytes {
                    return Ok(());
                }
                entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_access)
                    .map(|(key, entry)| (key.clone(), entry.path.clone()))
            };

            if let Some((key, path)) = victim {
                let _ = fs::remove_file(&path).await;
                let mut entries = self.entries.lock().expect("cache lock poisoned");
                entries.remove(&key);
            } else {
                return Ok(());
            }
        }
    }
}

pub async fn file_size(path: impl AsRef<Path>) -> anyhow::Result<u64> {
    Ok(fs::metadata(path).await.context("cache metadata")?.len())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}
