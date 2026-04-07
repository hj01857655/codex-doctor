use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::CodexLayout;

const MANIFEST_FILENAME: &str = "manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub backup_id: String,
    pub source_codex_home: PathBuf,
    pub created_at_unix_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSnapshot {
    pub backup_id: String,
    pub snapshot_dir: PathBuf,
    pub manifest: BackupManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackupPruneReport {
    pub removed_backup_ids: Vec<String>,
}

pub fn create_backup_snapshot(
    codex_home: &Path,
    backups_root: &Path,
) -> Result<BackupSnapshot, String> {
    create_backup_snapshot_with_sqlite_home(codex_home, backups_root, None)
}

pub fn create_backup_snapshot_with_sqlite_home(
    codex_home: &Path,
    backups_root: &Path,
    sqlite_home_override: Option<&Path>,
) -> Result<BackupSnapshot, String> {
    fs::create_dir_all(backups_root).map_err(|err| err.to_string())?;

    let layout =
        CodexLayout::from_codex_home_and_env(codex_home, sqlite_home_override.map(PathBuf::from));
    let created_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_millis();
    let backup_id = format!("backup-{created_at_unix_ms}");
    let snapshot_dir = unique_snapshot_dir(backups_root, &backup_id)?;

    fs::create_dir_all(&snapshot_dir).map_err(|err| err.to_string())?;

    copy_if_exists(&layout.config_toml, &snapshot_dir.join("config.toml"))?;
    copy_dir_if_exists(&layout.sessions_dir, &snapshot_dir.join("sessions"))?;
    copy_dir_if_exists(
        &layout.archived_sessions_dir,
        &snapshot_dir.join("archived_sessions"),
    )?;
    copy_if_exists(&layout.state_db, &snapshot_dir.join("state_5.sqlite"))?;
    copy_if_exists(&layout.logs_db, &snapshot_dir.join("logs_1.sqlite"))?;
    copy_if_exists(&layout.history_jsonl, &snapshot_dir.join("history.jsonl"))?;

    let manifest = BackupManifest {
        backup_id: snapshot_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "backup snapshot directory has invalid name".to_string())?
            .to_string(),
        source_codex_home: layout.codex_home,
        created_at_unix_ms,
    };
    write_manifest(&snapshot_dir, &manifest)?;

    Ok(BackupSnapshot {
        backup_id: manifest.backup_id.clone(),
        snapshot_dir,
        manifest,
    })
}

pub fn list_backups(backups_root: &Path) -> Result<Vec<BackupManifest>, String> {
    if !backups_root.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = Vec::new();
    for entry in fs::read_dir(backups_root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }

        let manifest_path = entry.path().join(MANIFEST_FILENAME);
        if !manifest_path.exists() {
            continue;
        }

        manifests.push(read_manifest(&manifest_path)?);
    }

    manifests.sort_by(|left, right| {
        right
            .created_at_unix_ms
            .cmp(&left.created_at_unix_ms)
            .then_with(|| right.backup_id.cmp(&left.backup_id))
    });

    Ok(manifests)
}

pub fn restore_backup(snapshot_dir: &Path, codex_home: &Path) -> Result<(), String> {
    restore_backup_with_sqlite_home(snapshot_dir, codex_home, None)
}

pub fn restore_backup_with_sqlite_home(
    snapshot_dir: &Path,
    codex_home: &Path,
    sqlite_home_override: Option<&Path>,
) -> Result<(), String> {
    let target_layout =
        CodexLayout::from_codex_home_and_env(codex_home, sqlite_home_override.map(PathBuf::from));

    restore_file(
        &snapshot_dir.join("config.toml"),
        &target_layout.config_toml,
    )?;
    restore_dir(&snapshot_dir.join("sessions"), &target_layout.sessions_dir)?;
    restore_dir(
        &snapshot_dir.join("archived_sessions"),
        &target_layout.archived_sessions_dir,
    )?;
    restore_file(
        &snapshot_dir.join("state_5.sqlite"),
        &target_layout.state_db,
    )?;
    restore_file(&snapshot_dir.join("logs_1.sqlite"), &target_layout.logs_db)?;
    restore_file(
        &snapshot_dir.join("history.jsonl"),
        &target_layout.history_jsonl,
    )?;

    Ok(())
}

pub fn prune_backups(backups_root: &Path, keep_latest: usize) -> Result<BackupPruneReport, String> {
    let backups = list_backups(backups_root)?;
    let mut removed_backup_ids = Vec::new();

    for manifest in backups.into_iter().skip(keep_latest) {
        let snapshot_dir = backups_root.join(&manifest.backup_id);
        if snapshot_dir.exists() {
            fs::remove_dir_all(&snapshot_dir).map_err(|err| err.to_string())?;
        }
        removed_backup_ids.push(manifest.backup_id);
    }

    Ok(BackupPruneReport { removed_backup_ids })
}

fn unique_snapshot_dir(backups_root: &Path, backup_id: &str) -> Result<PathBuf, String> {
    let primary = backups_root.join(backup_id);
    if !primary.exists() {
        return Ok(primary);
    }

    for suffix in 1..=999_u16 {
        let candidate = backups_root.join(format!("{backup_id}-{suffix}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("unable to create unique backup snapshot directory".to_string())
}

fn write_manifest(snapshot_dir: &Path, manifest: &BackupManifest) -> Result<(), String> {
    let content = serde_json::to_string_pretty(manifest).map_err(|err| err.to_string())?;
    fs::write(snapshot_dir.join(MANIFEST_FILENAME), content).map_err(|err| err.to_string())
}

fn read_manifest(path: &Path) -> Result<BackupManifest, String> {
    let content = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&content).map_err(|err| err.to_string())
}

fn copy_if_exists(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::copy(src, dst).map_err(|err| err.to_string())?;
    Ok(())
}

fn copy_dir_if_exists(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }

    fs::create_dir_all(dst).map_err(|err| err.to_string())?;
    for entry in fs::read_dir(src).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let destination = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_if_exists(&path, &destination)?;
        } else {
            copy_if_exists(&path, &destination)?;
        }
    }

    Ok(())
}

fn restore_file(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        fs::remove_file(dst).map_err(|err| err.to_string())?;
    }
    copy_if_exists(src, dst)
}

fn restore_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|err| err.to_string())?;
    }
    copy_dir_if_exists(src, dst)
}
