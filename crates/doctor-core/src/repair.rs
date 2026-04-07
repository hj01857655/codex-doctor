use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fs4::FileExt;
use rusqlite::{Connection, Error as SqliteError, ErrorCode, OpenFlags};

use crate::{
    create_backup_snapshot_with_sqlite_home, move_rollout_file, patch_root_model_provider,
    read_thread_by_id, rewrite_rollout_provider, upsert_thread_record, BackupSnapshot, CodexLayout,
    RepairAction, RepairPlan, RolloutRecord, SqliteThreadRecord, ThreadLocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairExecutionEntry {
    pub action: RepairAction,
    pub message: String,
    pub retryable: bool,
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
    execute_repair_plan_with_sqlite_home(codex_home, backups_root, plan, dry_run, None)
}

pub fn execute_repair_plan_with_sqlite_home(
    codex_home: &Path,
    backups_root: &Path,
    plan: &RepairPlan,
    dry_run: bool,
    sqlite_home_override: Option<&Path>,
) -> Result<RepairExecutionReport, String> {
    let layout =
        CodexLayout::from_codex_home_and_env(codex_home, sqlite_home_override.map(PathBuf::from));
    let backup = if dry_run || plan.actions.is_empty() {
        None
    } else {
        match create_backup_snapshot_with_sqlite_home(
            codex_home,
            backups_root,
            sqlite_home_override,
        ) {
            Ok(snapshot) => Some(snapshot),
            Err(message) if is_lock_message(&message) => {
                return Ok(RepairExecutionReport {
                    backup: None,
                    applied: Vec::new(),
                    skipped: plan
                        .actions
                        .iter()
                        .cloned()
                        .map(|action| RepairExecutionEntry {
                            action,
                            message: format!("backup blocked by locked resource: {message}"),
                            retryable: true,
                        })
                        .collect(),
                    failed: Vec::new(),
                });
            }
            Err(message) => return Err(message),
        }
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
                retryable: false,
            });
            continue;
        }

        match apply_action(&layout, action) {
            Ok(message) => report.applied.push(RepairExecutionEntry {
                action: action.clone(),
                message,
                retryable: false,
            }),
            Err(ApplyActionError::Retryable(message)) => {
                report.skipped.push(RepairExecutionEntry {
                    action: action.clone(),
                    message,
                    retryable: true,
                })
            }
            Err(ApplyActionError::Fatal(message)) => report.failed.push(RepairExecutionEntry {
                action: action.clone(),
                message,
                retryable: false,
            }),
        }
    }

    Ok(report)
}

enum ApplyActionError {
    Retryable(String),
    Fatal(String),
}

fn apply_action(layout: &CodexLayout, action: &RepairAction) -> Result<String, ApplyActionError> {
    match action {
        RepairAction::UpsertSqliteThreadMetadata { thread_id } => {
            ensure_sqlite_writable(&layout.state_db)?;
            let rollout = find_rollout_record(layout, thread_id)?.ok_or_else(|| {
                ApplyActionError::Fatal(format!("rollout record not found for thread {thread_id}"))
            })?;
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
            upsert_thread_record(&layout.state_db, &sqlite_record).map_err(classify_write_error)?;
            Ok(format!("upserted sqlite thread {}", rollout.thread_id))
        }
        RepairAction::RewriteRolloutSessionMeta {
            thread_id,
            provider,
        } => {
            let rollout = find_rollout_record(layout, thread_id)?.ok_or_else(|| {
                ApplyActionError::Fatal(format!("rollout record not found for thread {thread_id}"))
            })?;
            ensure_rollout_writable(&rollout.rollout_path)?;
            rewrite_rollout_provider(&rollout.rollout_path, provider)
                .map_err(classify_write_error)?;
            Ok(format!("rewrote rollout provider for {thread_id}"))
        }
        RepairAction::MoveRolloutToArchive { thread_id } => {
            ensure_sqlite_writable(&layout.state_db)?;
            move_rollout(layout, thread_id, &layout.archived_sessions_dir, true)
        }
        RepairAction::MoveRolloutToSessions { thread_id } => {
            ensure_sqlite_writable(&layout.state_db)?;
            move_rollout(layout, thread_id, &layout.sessions_dir, false)
        }
        RepairAction::PatchConfigModelProvider { provider } => {
            patch_root_model_provider(&layout.config_toml, provider)
                .map_err(classify_write_error)?;
            Ok(format!("patched config model_provider to {provider}"))
        }
        RepairAction::RebuildMissingIndexFromRollout { thread_id, .. } => {
            ensure_sqlite_writable(&layout.state_db)?;
            let rollout = find_rollout_record(layout, thread_id)?.ok_or_else(|| {
                ApplyActionError::Fatal(format!("rollout record not found for thread {thread_id}"))
            })?;
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
            upsert_thread_record(&layout.state_db, &sqlite_record).map_err(classify_write_error)?;
            Ok(format!("rebuilt sqlite index for {thread_id}"))
        }
    }
}

fn move_rollout(
    layout: &CodexLayout,
    thread_id: &str,
    destination_dir: &Path,
    archived: bool,
) -> Result<String, ApplyActionError> {
    let rollout = find_rollout_record(layout, thread_id)?.ok_or_else(|| {
        ApplyActionError::Fatal(format!("rollout record not found for thread {thread_id}"))
    })?;
    ensure_rollout_writable(&rollout.rollout_path)?;
    let new_path =
        move_rollout_file(&rollout.rollout_path, destination_dir).map_err(classify_write_error)?;

    if let Some(mut sqlite_record) = read_thread_by_id(&layout.state_db, thread_id)
        .map_err(|err| ApplyActionError::Fatal(err.to_string()))?
    {
        sqlite_record.rollout_path = new_path;
        sqlite_record.archived_at = if archived {
            Some(current_unix_timestamp())
        } else {
            None
        };
        upsert_thread_record(&layout.state_db, &sqlite_record).map_err(classify_write_error)?;
    }

    Ok(format!("moved rollout for {thread_id}"))
}

fn find_rollout_record(
    layout: &CodexLayout,
    thread_id: &str,
) -> Result<Option<RolloutRecord>, ApplyActionError> {
    find_rollout_in_dir(&layout.sessions_dir, ThreadLocation::Active, thread_id)?.map_or_else(
        || {
            find_rollout_in_dir(
                &layout.archived_sessions_dir,
                ThreadLocation::Archived,
                thread_id,
            )
        },
        |record| Ok(Some(record)),
    )
}

fn find_rollout_in_dir(
    dir: &Path,
    location: ThreadLocation,
    thread_id: &str,
) -> Result<Option<RolloutRecord>, ApplyActionError> {
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir).map_err(classify_io_error)? {
        let entry = entry.map_err(classify_io_error)?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if probe_rollout_lock(&path).map_err(classify_io_error)?
            && path_matches_thread(&path, thread_id)
        {
            return Err(ApplyActionError::Retryable(format!(
                "rollout file for {thread_id} is locked by another process"
            )));
        }

        let record =
            RolloutRecord::from_path(&path, location.clone()).map_err(ApplyActionError::Fatal)?;
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

fn path_matches_thread(path: &Path, thread_id: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(thread_id))
}

fn ensure_sqlite_writable(path: &Path) -> Result<(), ApplyActionError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(classify_sqlite_error)?;
    let _ = connection.busy_timeout(std::time::Duration::from_millis(0));
    connection
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .map_err(classify_sqlite_error)
}

fn ensure_rollout_writable(path: &Path) -> Result<(), ApplyActionError> {
    if probe_rollout_lock(path).map_err(classify_io_error)? {
        return Err(ApplyActionError::Retryable(format!(
            "rollout file {} is locked by another process",
            path.display()
        )));
    }
    Ok(())
}

fn probe_rollout_lock(path: &Path) -> Result<bool, std::io::Error> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            Ok(false)
        }
        Err(err) if is_lock_io_error(&err) => Ok(true),
        Err(err) => Err(err),
    }
}

fn classify_write_error(message: String) -> ApplyActionError {
    if is_lock_message(&message) {
        ApplyActionError::Retryable(message)
    } else {
        ApplyActionError::Fatal(message)
    }
}

fn classify_sqlite_error(err: SqliteError) -> ApplyActionError {
    if is_lock_sqlite_error(&err) {
        ApplyActionError::Retryable(err.to_string())
    } else {
        ApplyActionError::Fatal(err.to_string())
    }
}

fn classify_io_error(err: std::io::Error) -> ApplyActionError {
    if is_lock_io_error(&err) {
        ApplyActionError::Retryable(err.to_string())
    } else {
        ApplyActionError::Fatal(err.to_string())
    }
}

fn is_lock_sqlite_error(err: &SqliteError) -> bool {
    match err {
        SqliteError::SqliteFailure(inner, _) => {
            matches!(
                inner.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            )
        }
        _ => is_lock_message(&err.to_string()),
    }
}

fn is_lock_io_error(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::PermissionDenied
    ) || matches!(err.raw_os_error(), Some(32 | 33))
        || is_lock_message(&err.to_string())
}

fn is_lock_message(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    text.contains("database is locked")
        || text.contains("database is busy")
        || text.contains("used by another process")
        || text.contains("cannot access the file")
        || text.contains("resource temporarily unavailable")
        || text.contains("would block")
        || text.contains("os error 32")
        || text.contains("os error 33")
}
