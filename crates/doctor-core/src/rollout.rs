use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::{RolloutRecord, RolloutSessionMeta, ThreadLocation};

#[derive(Debug, Deserialize)]
struct RolloutLine {
    #[serde(rename = "type")]
    line_type: String,
    payload: SessionMetaPayload,
}

#[derive(Debug, Deserialize)]
struct SessionMetaPayload {
    id: String,
    timestamp: String,
    cwd: PathBuf,
    model_provider: Option<String>,
}

impl RolloutRecord {
    pub fn from_path(path: &Path, location: ThreadLocation) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
        let line = content
            .lines()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| "rollout file is empty".to_string())?;

        let parsed: RolloutLine = serde_json::from_str(line).map_err(|err| err.to_string())?;
        if parsed.line_type != "session_meta" {
            return Err("first rollout line is not session_meta".to_string());
        }

        Ok(Self {
            thread_id: parsed.payload.id,
            rollout_path: path.to_path_buf(),
            session_meta: RolloutSessionMeta {
                provider: parsed.payload.model_provider,
                cwd: parsed.payload.cwd,
                timestamp: parsed.payload.timestamp,
            },
            archived: matches!(location, ThreadLocation::Archived),
            location,
        })
    }
}

pub fn rewrite_rollout_provider(path: &Path, provider: &str) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut lines = content.lines();
    let first_line = lines
        .next()
        .ok_or_else(|| "rollout file is empty".to_string())?;

    let mut value: Value = serde_json::from_str(first_line).map_err(|err| err.to_string())?;
    value["payload"]["model_provider"] = Value::String(provider.to_string());

    let mut updated = serde_json::to_string(&value).map_err(|err| err.to_string())?;
    for line in lines {
        updated.push('\n');
        updated.push_str(line);
    }
    if content.ends_with('\n') {
        updated.push('\n');
    }

    fs::write(path, updated).map_err(|err| err.to_string())
}

pub fn move_rollout_file(path: &Path, destination_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(destination_dir).map_err(|err| err.to_string())?;
    let file_name = path
        .file_name()
        .ok_or_else(|| "rollout path is missing file name".to_string())?;
    let destination = destination_dir.join(file_name);

    if path == destination {
        return Ok(destination);
    }
    if destination.exists() {
        fs::remove_file(&destination).map_err(|err| err.to_string())?;
    }

    fs::rename(path, &destination).map_err(|err| err.to_string())?;
    Ok(destination)
}
