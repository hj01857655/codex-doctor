use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{RepairAction, RepairExecutionReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairHistoryEntry {
    pub timestamp: i64,
    pub codex_home: PathBuf,
    pub actions_applied: usize,
    pub actions_skipped: usize,
    pub actions_failed: usize,
    pub backup_id: Option<String>,
    pub actions: Vec<RepairActionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairActionRecord {
    pub action_type: String,
    pub thread_id: Option<String>,
    pub details: String,
    pub status: ActionStatus,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    Applied,
    Skipped,
    Failed,
}

pub fn save_repair_history(
    history_dir: &Path,
    codex_home: &Path,
    report: &RepairExecutionReport,
    _actions: &[RepairAction],
) -> Result<PathBuf, String> {
    fs::create_dir_all(history_dir).map_err(|err| err.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_secs() as i64;

    let backup_id = report.backup.as_ref().map(|b| b.backup_id.clone());

    let mut action_records = Vec::new();
    for entry in &report.applied {
        action_records.push(RepairActionRecord {
            action_type: action_type_name(&entry.action),
            thread_id: extract_thread_id(&entry.action),
            details: entry.message.clone(),
            status: ActionStatus::Applied,
            retryable: entry.retryable,
        });
    }
    for entry in &report.skipped {
        action_records.push(RepairActionRecord {
            action_type: action_type_name(&entry.action),
            thread_id: extract_thread_id(&entry.action),
            details: entry.message.clone(),
            status: ActionStatus::Skipped,
            retryable: entry.retryable,
        });
    }
    for entry in &report.failed {
        action_records.push(RepairActionRecord {
            action_type: action_type_name(&entry.action),
            thread_id: extract_thread_id(&entry.action),
            details: entry.message.clone(),
            status: ActionStatus::Failed,
            retryable: entry.retryable,
        });
    }

    let history_entry = RepairHistoryEntry {
        timestamp,
        codex_home: codex_home.to_path_buf(),
        actions_applied: report.applied.len(),
        actions_skipped: report.skipped.len(),
        actions_failed: report.failed.len(),
        backup_id,
        actions: action_records,
    };

    let filename = format!("repair-{}.json", timestamp);
    let history_file = history_dir.join(&filename);
    let json = serde_json::to_string_pretty(&history_entry).map_err(|err| err.to_string())?;
    fs::write(&history_file, json).map_err(|err| err.to_string())?;

    Ok(history_file)
}

pub fn list_repair_history(history_dir: &Path) -> Result<Vec<RepairHistoryEntry>, String> {
    if !history_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(history_dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let content = fs::read_to_string(&path).map_err(|err| err.to_string())?;
            let history_entry: RepairHistoryEntry =
                serde_json::from_str(&content).map_err(|err| err.to_string())?;
            entries.push(history_entry);
        }
    }

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(entries)
}

fn action_type_name(action: &RepairAction) -> String {
    match action {
        RepairAction::RebuildMissingIndexFromRollout { .. } => {
            "rebuild_missing_index_from_rollout".to_string()
        }
        RepairAction::UpsertSqliteThreadMetadata { .. } => {
            "upsert_sqlite_thread_metadata".to_string()
        }
        RepairAction::MoveRolloutToArchive { .. } => "move_rollout_to_archive".to_string(),
        RepairAction::MoveRolloutToSessions { .. } => "move_rollout_to_sessions".to_string(),
        RepairAction::RewriteRolloutSessionMeta { .. } => {
            "rewrite_rollout_session_meta".to_string()
        }
        RepairAction::PatchConfigModelProvider { .. } => "patch_config_model_provider".to_string(),
    }
}

fn extract_thread_id(action: &RepairAction) -> Option<String> {
    match action {
        RepairAction::RebuildMissingIndexFromRollout { thread_id, .. }
        | RepairAction::UpsertSqliteThreadMetadata { thread_id }
        | RepairAction::MoveRolloutToArchive { thread_id }
        | RepairAction::MoveRolloutToSessions { thread_id }
        | RepairAction::RewriteRolloutSessionMeta { thread_id, .. } => Some(thread_id.clone()),
        RepairAction::PatchConfigModelProvider { .. } => None,
    }
}
