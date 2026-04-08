use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_TTL_SECS: u64 = 3600; // 1 hour

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Cache {
    pub users: Option<CachedData>,
    pub channels: Option<CachedData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedData {
    pub data: Value,
    pub timestamp: u64,
}

fn cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("slack")
        .join("cache.json")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn load_cache() -> Cache {
    let path = cache_path();
    if !path.exists() {
        return Cache::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn save_cache(cache: &Cache) -> Result<()> {
    let path = cache_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, serde_json::to_string(cache)?)?;
    Ok(())
}

fn get_cached(entry: &Option<CachedData>) -> Option<Value> {
    entry.as_ref().and_then(|c| {
        if now() - c.timestamp < CACHE_TTL_SECS {
            Some(c.data.clone())
        } else {
            None
        }
    })
}

fn set_cached(entry: &mut Option<CachedData>, data: Value) {
    *entry = Some(CachedData {
        data,
        timestamp: now(),
    });
}

pub fn get_users(cache: &Cache) -> Option<Value> {
    get_cached(&cache.users)
}

pub fn set_users(cache: &mut Cache, data: Value) {
    set_cached(&mut cache.users, data);
}

pub fn get_channels(cache: &Cache) -> Option<Value> {
    get_cached(&cache.channels)
}

pub fn set_channels(cache: &mut Cache, data: Value) {
    set_cached(&mut cache.channels, data);
}
