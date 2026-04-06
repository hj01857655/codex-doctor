use doctor_core::{
    list_repair_history, save_repair_history, BackupManifest, BackupSnapshot, RepairAction,
    RepairExecutionEntry, RepairExecutionReport,
};
use std::path::PathBuf;
use tempfile::tempdir;

fn sample_execution_report_with_entries(history_root: &std::path::Path) -> RepairExecutionReport {
    let backup_id = "backup-123".to_string();
    let snapshot_dir = history_root.join(&backup_id);

    RepairExecutionReport {
        backup: Some(BackupSnapshot {
            backup_id: backup_id.clone(),
            snapshot_dir: snapshot_dir.clone(),
            manifest: BackupManifest {
                backup_id,
                source_codex_home: PathBuf::from("/test/codex"),
                created_at_unix_ms: 1_700_000_000_000,
            },
        }),
        applied: vec![RepairExecutionEntry {
            action: RepairAction::PatchConfigModelProvider {
                provider: "openai".to_string(),
            },
            message: "patched config model_provider to openai".to_string(),
        }],
        skipped: vec![RepairExecutionEntry {
            action: RepairAction::MoveRolloutToArchive {
                thread_id: "thread-skipped".to_string(),
            },
            message: "dry-run".to_string(),
        }],
        failed: vec![RepairExecutionEntry {
            action: RepairAction::RewriteRolloutSessionMeta {
                thread_id: "thread-failed".to_string(),
                provider: "anthropic".to_string(),
            },
            message: "rewrite failed".to_string(),
        }],
    }
}

#[test]
fn saves_and_loads_repair_history() {
    let history_dir = tempdir().expect("create tempdir");
    let codex_home = PathBuf::from("/test/codex");

    let report = RepairExecutionReport {
        backup: None,
        applied: vec![],
        skipped: vec![],
        failed: vec![],
    };

    let history_file =
        save_repair_history(history_dir.path(), &codex_home, &report, &[]).expect("save history");

    assert!(history_file.exists());

    let entries = list_repair_history(history_dir.path()).expect("list history");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].codex_home, codex_home);
    assert_eq!(entries[0].actions_applied, 0);
}

#[test]
fn lists_multiple_history_entries_in_reverse_chronological_order() {
    let history_dir = tempdir().expect("create tempdir");
    let codex_home = PathBuf::from("/test/codex");

    let report = RepairExecutionReport {
        backup: None,
        applied: vec![],
        skipped: vec![],
        failed: vec![],
    };

    save_repair_history(history_dir.path(), &codex_home, &report, &[]).expect("save history 1");
    std::thread::sleep(std::time::Duration::from_secs(1));
    save_repair_history(history_dir.path(), &codex_home, &report, &[]).expect("save history 2");

    let entries = list_repair_history(history_dir.path()).expect("list history");
    assert_eq!(entries.len(), 2);
    assert!(entries[0].timestamp >= entries[1].timestamp);
}

#[test]
fn returns_empty_list_for_nonexistent_directory() {
    let history_dir = PathBuf::from("/nonexistent/history");
    let entries = list_repair_history(&history_dir).expect("list history");
    assert_eq!(entries.len(), 0);
}

#[test]
fn save_repair_history_persists_backup_id_when_present() {
    let history_dir = tempdir().expect("create tempdir");
    let codex_home = PathBuf::from("/test/codex");
    let report = sample_execution_report_with_entries(history_dir.path());

    save_repair_history(history_dir.path(), &codex_home, &report, &[]).expect("save history");

    let entries = list_repair_history(history_dir.path()).expect("list history");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].backup_id.as_deref(), Some("backup-123"));
    assert_eq!(entries[0].actions_applied, 1);
    assert_eq!(entries[0].actions_skipped, 1);
    assert_eq!(entries[0].actions_failed, 1);
}

#[test]
fn save_repair_history_persists_action_statuses() {
    let history_dir = tempdir().expect("create tempdir");
    let codex_home = PathBuf::from("/test/codex");
    let report = sample_execution_report_with_entries(history_dir.path());

    save_repair_history(history_dir.path(), &codex_home, &report, &[]).expect("save history");

    let entries = list_repair_history(history_dir.path()).expect("list history");
    let entry = &entries[0];

    assert_eq!(entry.actions.len(), 3);
    assert!(entry.actions.iter().any(|action| {
        matches!(action.status, doctor_core::ActionStatus::Applied)
            && action.action_type == "patch_config_model_provider"
    }));
    assert!(entry.actions.iter().any(|action| {
        matches!(action.status, doctor_core::ActionStatus::Skipped)
            && action.action_type == "move_rollout_to_archive"
            && action.thread_id.as_deref() == Some("thread-skipped")
    }));
    assert!(entry.actions.iter().any(|action| {
        matches!(action.status, doctor_core::ActionStatus::Failed)
            && action.action_type == "rewrite_rollout_session_meta"
            && action.thread_id.as_deref() == Some("thread-failed")
    }));
}
