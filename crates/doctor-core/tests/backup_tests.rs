use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use doctor_core::{create_backup_snapshot, list_backups, prune_backups, restore_backup, CodexLayout};
use rusqlite::Connection;
use tempfile::tempdir;

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create destination");
    for entry in fs::read_dir(src).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let destination = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &destination);
        } else {
            fs::copy(&path, &destination).expect("copy file");
        }
    }
}

fn prepare_codex_home() -> tempfile::TempDir {
    let temp = tempdir().expect("create tempdir");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("backup")
        .join("sample-codex");

    copy_dir_recursive(&source, temp.path());
    create_sqlite_file(&CodexLayout::from_codex_home(temp.path()).state_db);
    create_sqlite_file(&CodexLayout::from_codex_home(temp.path()).logs_db);
    temp
}

fn create_sqlite_file(path: &Path) {
    let connection = Connection::open(path).expect("open sqlite file");
    connection
        .execute_batch("CREATE TABLE meta (id INTEGER PRIMARY KEY, value TEXT NOT NULL);")
        .expect("create sqlite schema");
}

#[test]
fn create_backup_snapshot_copies_layout_and_writes_manifest() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());

    let snapshot = create_backup_snapshot(codex_home.path(), backups_root.path())
        .expect("create backup snapshot");

    assert!(snapshot.snapshot_dir.join("manifest.json").exists());
    assert!(snapshot.snapshot_dir.join("config.toml").exists());
    assert!(snapshot.snapshot_dir.join("sessions").exists());
    assert!(snapshot.snapshot_dir.join("archived_sessions").exists());
    assert!(snapshot.snapshot_dir.join("history.jsonl").exists());
    assert!(snapshot.snapshot_dir.join("state_5.sqlite").exists());
    assert!(snapshot.snapshot_dir.join("logs_2.sqlite").exists());

    let backups = list_backups(backups_root.path()).expect("list backups");
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].backup_id, snapshot.backup_id);
    assert_eq!(backups[0].source_codex_home, layout.codex_home);
}

#[test]
fn restore_backup_restores_deleted_files() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());
    let snapshot = create_backup_snapshot(codex_home.path(), backups_root.path())
        .expect("create backup snapshot");

    fs::remove_file(&layout.config_toml).expect("remove config");
    fs::remove_file(&layout.history_jsonl).expect("remove history");
    fs::remove_file(&layout.state_db).expect("remove state db");
    fs::remove_dir_all(&layout.sessions_dir).expect("remove sessions");
    fs::remove_dir_all(&layout.archived_sessions_dir).expect("remove archived sessions");

    restore_backup(&snapshot.snapshot_dir, codex_home.path()).expect("restore backup");

    assert!(layout.config_toml.exists());
    assert!(layout.history_jsonl.exists());
    assert!(layout.state_db.exists());
    assert!(layout.logs_db.exists());
    assert!(layout
        .sessions_dir
        .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl")
        .exists());
    assert!(layout
        .archived_sessions_dir
        .join("rollout-2026-01-26T09-00-00-00000000-0000-0000-0000-000000000456.jsonl")
        .exists());
}

#[test]
fn prune_backups_keeps_only_the_newest_snapshots() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");

    let first = create_backup_snapshot(codex_home.path(), backups_root.path())
        .expect("create first backup");
    thread::sleep(Duration::from_millis(20));
    let second = create_backup_snapshot(codex_home.path(), backups_root.path())
        .expect("create second backup");
    thread::sleep(Duration::from_millis(20));
    let third = create_backup_snapshot(codex_home.path(), backups_root.path())
        .expect("create third backup");

    let pruned = prune_backups(backups_root.path(), 2).expect("prune backups");

    assert_eq!(pruned.removed_backup_ids, vec![first.backup_id.clone()]);

    let remaining = list_backups(backups_root.path()).expect("list remaining backups");
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].backup_id, third.backup_id);
    assert_eq!(remaining[1].backup_id, second.backup_id);
    assert!(!backups_root.path().join(first.backup_id).exists());
    assert!(backups_root.path().join(second.backup_id).exists());
    assert!(backups_root.path().join(third.backup_id).exists());
}
