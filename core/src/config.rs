use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_PORT: u16 = 17_820;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    /// Bearer token web apps connect with. Rotating it disconnects every one of them.
    pub token: String,
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).expect("OS random source");
    hex::encode(bytes)
}

fn path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

/// Reads the stored config, creating one with a fresh token on first run.
pub fn load(dir: &Path, port: u16) -> Result<Config, String> {
    fs::create_dir_all(dir).map_err(|e| format!("data dir: {e}"))?;
    let stored = fs::read(path(dir))
        .ok()
        .and_then(|b| serde_json::from_slice::<Config>(&b).ok());
    let mut config = stored.unwrap_or_else(|| Config { port, token: new_token() });
    if port != 0 {
        config.port = port;
    }
    if config.port == 0 {
        config.port = DEFAULT_PORT;
    }
    save(dir, &config)?;
    Ok(config)
}

pub fn save(dir: &Path, config: &Config) -> Result<(), String> {
    let tmp = dir.join("config.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(config).unwrap()).map_err(|e| format!("config: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(tmp, path(dir)).map_err(|e| format!("config: {e}"))
}
