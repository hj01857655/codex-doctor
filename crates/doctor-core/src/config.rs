use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::RootConfigSnapshot;

#[derive(Debug, Deserialize, Serialize)]
struct RootConfigToml {
    model_provider: Option<String>,
}

pub fn read_root_config_snapshot(path: &Path) -> Result<RootConfigSnapshot, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let parsed: RootConfigToml = toml::from_str(&content).map_err(|err| err.to_string())?;

    Ok(RootConfigSnapshot {
        model_provider: parsed.model_provider,
    })
}

pub fn patch_root_model_provider(path: &Path, provider: &str) -> Result<(), String> {
    let content = toml::to_string(&RootConfigToml {
        model_provider: Some(provider.to_string()),
    })
    .map_err(|err| err.to_string())?;

    fs::write(path, content).map_err(|err| err.to_string())
}
