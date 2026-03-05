use std::fs;
use std::path::PathBuf;

use directories::BaseDirs;
use serde::{de::DeserializeOwned, Serialize};

pub fn config_dir() -> Result<PathBuf, String> {
    let base = BaseDirs::new().ok_or_else(|| "Unable to resolve home directory".to_string())?;
    Ok(base.home_dir().join(".config").join("gtui"))
}

pub fn data_dir() -> Result<PathBuf, String> {
    let base = BaseDirs::new().ok_or_else(|| "Unable to resolve home directory".to_string())?;
    Ok(base.home_dir().join(".local").join("share").join("gtui"))
}

pub fn ensure_config_dir() -> Result<PathBuf, String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn ensure_data_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create data dir {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn credentials_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("credentials.json"))
}

pub fn token_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("token.json"))
}

pub fn gmail_token_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("gmail_token.json"))
}

pub fn calendar_token_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("calendar_token.json"))
}

pub fn cache_db_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("cache.db"))
}

pub fn read_json<T: DeserializeOwned>(path: &PathBuf) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("Failed reading {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed parsing {}: {e}", path.display()))
}

pub fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(value).map_err(|e| format!("JSON encode error: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("Failed writing {}: {e}", path.display()))
}
