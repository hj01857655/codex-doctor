use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    create_backup_snapshot, move_rollout_file, patch_root_model_provider, read_thread_by_id,
    rewrite_rollout_provider, upsert_thread_record, BackupSnapshot, CodexLayout, RepairAction,
    RepairPlan, RolloutRecord, SqliteThreadRecord, ThreadLocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairExecutionEntry {
    pub action: RepairAction,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepairExecutionReport {
    pub backup: Option<BackupSnapshot>,
    pub applied: Vec<RepairExecutionEntry>,
    pub skipped: Vec<RepairExecutionEntry>,
    pub failed: Vec<RepairExecutionEntry>,
}

pub fn execute_repair_plan(
    codex_home: &Path,
    backups_root: &Path,
    plan: &RepairPlan,
    dry_run: bool,
) -> Result<RepairExecutionReport, String> {
    let layout = CodexLayout::from_codex_home(codex_home);
    let backup = if dry_run || plan.actions.is_empty() {
        None
    } else {
        Some(create_backup_snapshot(codex_home, backups_root)?)
    };

    let mut report = RepairExecutionReport {
        backup,
        ..RepairExecutionReport::default()
    };

    for action in &plan.actions {
        if dry_run {
            report.skipped.push(RepairExecutionEntry {
                action: action.clone(),
                message: "dry-run".to_string(),
            });
            continue;
        }

        match apply_action(&layout, action) {
            Ok(message) => report.applied.push(RepairExecutionEntry {
                action: action.clone(),
                message,
            }),
            Err(message) => report.failed.push(RepairExecutionEntry {
                action: action.clone(),
                message,
            }),
        }
    }

    Ok(report)
}

fn apply_action(layout: &CodexLayout, action: &RepairAction) -> Result<String, String> {
    match action {
        RepairAction::UpsertSqliteThreadMetadata { thread_id } => {
            let rollout = find_rollout_record(layout, thread_id)?
                .ok_or_else(|| format!("rollout record not found for thread {thread_id}"))?;
            let sqlite_record = SqliteThreadRecord {
                id: rollout.thread_id.clone(),
                rollout_path: rollout.rollout_path.clone(),
                model_provider: rollout
                    .session_meta
                    .provider
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                archived_at: archived_timestamp(&rollout),
                cwd: rollout.session_meta.cwd.clone(),
            };
            upsert_thread_record(&layout.state_db, &sqlite_record)?;
            Ok(format!("upserted sqlite thread {}", rollout.thread_id))
        }
        RepairAction::RewriteRolloutSessionMeta { thread_id, provider } => {
            let rollout = find_rollout_record(layout, thread_id)?
                .ok_or_else(|| format!("rollout record not found for thread {thread_id}"))?;
            rewrite_rollout_provider(&rollout.rollout_path, provider)?;
            Ok(format!("rewrote rollout provider for {thread_id}"))
        }
        RepairAction::MoveRolloutToArchive { thread_id } => {
            move_rollout(layout, thread_id, &layout.archived_sessions_dir, true)
        }
        RepairAction::MoveRolloutToSessions { thread_id } => {
            move_rollout(layout, thread_id, &layout.sessions_dir, false)
        }
        RepairAction::PatchConfigModelProvider { provider } => {
            patch_root_model_provider(&layout.config_toml, provider)?;
            Ok(format!("patched config model_provider to {provider}"))
        }
        RepairAction::RebuildMissingIndexFromRollout { thread_id, .. } => {
            let rollout = find_rollout_record(layout, thread_id)?
                .ok_or_else(|| format!("rollout record not found for thread {thread_id}"))?;
            let sqlite_record = SqliteThreadRecord {
                id: rollout.thread_id.clone(),
                rollout_path: rollout.rollout_path.clone(),
                model_provider: rollout
                    .session_meta
                    .provider
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                archived_at: archived_timestamp(&rollout),
                cwd: rollout.session_meta.cwd.clone(),
            };
            upsert_thread_record(&layout.state_db, &sqlite_record)?;
            Ok(format!("rebuilt sqlite index for {thread_id}"))
        }
    }
}

fn move_rollout(
    layout: &CodexLayout,
    thread_id: &str,
    destination_dir: &Path,
    archived: bool,
) -> Result<String, String> {
    let rollout = find_rollout_record(layout, thread_id)?
        .ok_or_else(|| format!("rollout record not found for thread {thread_id}"))?;
    let new_path = move_rollout_file(&rollout.rollout_path, destination_dir)?;

    if let Some(mut sqlite_record) = read_thread_by_id(&layout.state_db, thread_id)
        .map_err(|err| err.to_string())?
    {
        sqlite_record.rollout_path = new_path;
        sqlite_record.archived_at = if archived {
            Some(current_unix_timestamp())
        } else {
            None
        };
        upsert_thread_record(&layout.state_db, &sqlite_record)?;
    }

    Ok(format!("moved rollout for {thread_id}"))
}

fn find_rollout_record(layout: &CodexLayout, thread_id: &str) -> Result<Option<RolloutRecord>, String> {
    find_rollout_in_dir(&layout.sessions_dir, ThreadLocation::Active, thread_id)?.map_or_else(
        || find_rollout_in_dir(&layout.archived_sessions_dir, ThreadLocation::Archived, thread_id),
        |record| Ok(Some(record)),
    )
}

fn find_rollout_in_dir(
    dir: &Path,
    location: ThreadLocation,
    thread_id: &str,
) -> Result<Option<RolloutRecord>, String> {
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let record = RolloutRecord::from_path(&path, location.clone())?;
        if record.thread_id == thread_id {
            return Ok(Some(record));
        }
    }

    Ok(None)
}

fn archived_timestamp(rollout: &RolloutRecord) -> Option<i64> {
    if matches!(rollout.location, ThreadLocation::Archived) || rollout.archived {
        Some(current_unix_timestamp())
    } else {
        None
    }
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}
