use std::fs;
use std::path::{Path, PathBuf};

use doctor_core::{create_backup_snapshot, save_repair_history, RepairExecutionReport};
use gui::{load_dashboard_view_model, CodexDoctorApp};
use rusqlite::{params, Connection};
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
    create_threads_table(&temp.path().join("state_5.sqlite"));
    let archived_rollout = temp
        .path()
        .join("archived_sessions")
        .join("rollout-2026-01-26T09-00-00-00000000-0000-0000-0000-000000000456.jsonl");
    fs::remove_file(&archived_rollout).expect("remove archived rollout fixture");
    insert_thread(
        &temp.path().join("state_5.sqlite"),
        "00000000-0000-0000-0000-000000000123",
        &temp
            .path()
            .join("sessions")
            .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl"),
        "openai",
        "/workspace/active",
        None,
    );
    temp
}

fn create_threads_table(path: &Path) {
    let connection = Connection::open(path).expect("open sqlite");
    connection
        .execute_batch(
            "
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                agent_nickname TEXT,
                agent_role TEXT,
                agent_path TEXT,
                model_provider TEXT NOT NULL,
                model TEXT,
                reasoning_effort TEXT,
                cwd TEXT NOT NULL,
                cli_version TEXT NOT NULL,
                title TEXT NOT NULL,
                sandbox_policy TEXT NOT NULL,
                approval_mode TEXT NOT NULL,
                tokens_used INTEGER NOT NULL,
                first_user_message TEXT NOT NULL,
                archived_at INTEGER,
                git_sha TEXT,
                git_branch TEXT,
                git_origin_url TEXT
            );
            ",
        )
        .expect("create threads table");
}

fn insert_thread(
    path: &Path,
    id: &str,
    rollout_path: &Path,
    provider: &str,
    cwd: &str,
    archived_at: Option<i64>,
) {
    let connection = Connection::open(path).expect("open sqlite");
    connection
        .execute(
            "
            INSERT INTO threads (
                id, rollout_path, created_at, updated_at, source, agent_nickname, agent_role,
                agent_path, model_provider, model, reasoning_effort, cwd, cli_version, title,
                sandbox_policy, approval_mode, tokens_used, first_user_message, archived_at,
                git_sha, git_branch, git_origin_url
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL, ?6, NULL, NULL, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL, NULL, NULL)
            ",
            params![
                id,
                rollout_path.display().to_string(),
                1_700_000_000_i64,
                1_700_000_100_i64,
                "cli",
                provider,
                cwd,
                "0.1.0",
                "Example thread",
                "read-only",
                "on-request",
                123_i64,
                "hello",
                archived_at,
            ],
        )
        .expect("insert thread");
}

#[test]
fn gui_layer_builds_summary_view_model_from_core_scan() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let view_model = load_dashboard_view_model(codex_home.path()).expect("load dashboard");

    assert!(view_model
        .summary_items
        .iter()
        .any(|item| item.label == "Active sessions" && item.value == "1"));
    assert!(view_model
        .summary_items
        .iter()
        .any(|item| item.label == "Problems" && item.value == "1"));
    assert!(view_model
        .problems
        .iter()
        .any(|problem| problem.code == "missing_root_model_provider"));
    assert!(view_model
        .preview_actions
        .iter()
        .any(|action| action == "patch_config_model_provider"));
}

#[test]
fn new_with_codex_home_prefetches_dashboard() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let app = CodexDoctorApp::new(codex_home.path().display().to_string());

    let dashboard = app.dashboard.as_ref().expect("dashboard preloaded");
    assert_eq!(
        dashboard.codex_home,
        codex_home.path().display().to_string()
    );
    assert!(app.last_error.is_none());
}

#[test]
fn new_with_invalid_codex_home_records_error() {
    let app = CodexDoctorApp::new("Z:\\definitely-missing-codex-home".to_string());

    assert!(app.dashboard.is_none());
    assert!(app.last_error.is_some());
}

#[test]
fn refresh_updates_dashboard_state_from_codex_home_input() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(String::new());
    app.set_codex_home_input(codex_home.path().display().to_string());
    app.refresh().expect("refresh dashboard");

    let dashboard = app.dashboard.as_ref().expect("dashboard state");
    assert_eq!(
        dashboard.codex_home,
        codex_home.path().display().to_string()
    );
    assert!(dashboard
        .summary_items
        .iter()
        .any(|item| item.label == "Problems" && item.value == "1"));
    assert!(app.preview_summary.is_empty());
}

#[test]
fn preview_action_updates_preview_summary_after_refresh() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.refresh().expect("refresh dashboard");
    app.preview_repair().expect("preview repair");

    assert!(app
        .preview_actions()
        .iter()
        .any(|action| action == "patch_config_model_provider"));
    assert!(app.preview_summary.contains("patch_config_model_provider"));
    assert_eq!(app.status_message, "Previewed: 1");
    assert_eq!(app.preview_repair_label(), "Preview repair");
}

#[test]
fn execute_action_runs_repair_and_updates_status() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.refresh().expect("refresh dashboard");
    app.execute_repair().expect("execute repair");

    let config = fs::read_to_string(codex_home.path().join("config.toml")).expect("read config");
    assert!(config.contains("model_provider = \"openai\""));
    assert!(app.status_message.contains("Applied: 1"));
    assert!(codex_home.path().join(".codex-doctor-backups").exists());
    assert!(app
        .dashboard
        .as_ref()
        .expect("dashboard after execute")
        .problems
        .iter()
        .all(|problem| problem.code != "missing_root_model_provider"));
    assert_eq!(app.execute_repair_label(), "Execute repair");
    assert!(!app.backups.is_empty());
    assert!(!app.history.is_empty());
    assert_eq!(app.last_operation_title.as_deref(), Some("Last repair"));
    assert!(app.last_operation_at.is_some());
    assert!(!app.last_execution.is_empty());
    assert!(app
        .last_execution
        .iter()
        .any(|action| matches!(action.status, doctor_core::ActionStatus::Applied)));
}

#[test]
fn refresh_keeps_last_execution_details() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    let before = app.last_execution.clone();
    let before_timestamp = app.last_operation_at;

    app.refresh().expect("refresh dashboard");

    assert_eq!(app.last_execution, before);
    assert_eq!(app.last_operation_at, before_timestamp);
}

#[test]
fn load_backups_populates_backup_selection_state() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_backups().expect("load backups");

    assert!(!app.backups.is_empty());
    assert_eq!(app.selected_backup, None);
}

#[test]
fn refresh_backups_reloads_backup_list() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_backups().expect("load backups");
    let before = app.backups.len();

    let backups_root = codex_home.path().join(".codex-doctor-backups");
    create_backup_snapshot(codex_home.path(), &backups_root).expect("create backup snapshot");

    app.refresh_backups().expect("refresh backups");

    assert_eq!(app.backups.len(), before + 1);
    assert_eq!(app.selected_backup, None);
    assert!(app.status_message.contains("Loaded"));
}

#[test]
fn load_history_reads_saved_repair_entries() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_history().expect("load history");

    assert!(!app.history.is_empty());
    assert_eq!(app.selected_history, None);
    assert!(app.history[0].actions_applied >= 1);
}

#[test]
fn refresh_history_reloads_history_list() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_history().expect("load history");
    let before = app.history.len();

    let history_dir = codex_home.path().join(".codex-doctor").join("history");
    std::thread::sleep(std::time::Duration::from_secs(1));
    save_repair_history(
        &history_dir,
        codex_home.path(),
        &RepairExecutionReport::default(),
        &[],
    )
    .expect("save history");

    app.refresh_history().expect("refresh history");

    assert_eq!(app.history.len(), before + 1);
    assert_eq!(app.selected_history, None);
    assert!(app.status_message.contains("Loaded"));
}

#[test]
fn restore_selected_backup_restores_previous_config_state() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_backups().expect("load backups");
    app.selected_backup = Some(0);

    fs::write(
        codex_home.path().join("config.toml"),
        "model_provider = \"broken\"\n",
    )
    .expect("mutate config after backup");

    app.restore_selected_backup()
        .expect("restore selected backup");

    let restored = fs::read_to_string(codex_home.path().join("config.toml")).expect("read config");
    assert!(!restored.contains("model_provider = \"broken\""));
    assert!(app.status_message.contains("Restored backup:"));
    assert!(app
        .dashboard
        .as_ref()
        .expect("dashboard after restore")
        .problems
        .iter()
        .any(|problem| problem.code == "missing_root_model_provider"));
    assert!(!app.backups.is_empty());
    assert!(!app.history.is_empty());
    assert_eq!(app.last_operation_title.as_deref(), Some("Last restore"));
    assert!(app.last_operation_at.is_some());
    assert!(app.last_execution.is_empty());
}

#[test]
fn load_backups_handles_missing_directory_gracefully() {
    let codex_home = prepare_codex_home();
    let backups_dir = codex_home.path().join(".codex-doctor-backups");
    if backups_dir.exists() {
        fs::remove_dir_all(&backups_dir).expect("remove backups dir");
    }

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.load_backups().expect("load backups");

    assert!(app.backups.is_empty());
    assert_eq!(app.selected_backup, None);
    assert!(app.last_error.is_none());
}

#[test]
fn load_history_handles_missing_history_gracefully() {
    let codex_home = prepare_codex_home();
    let history_dir = codex_home.path().join(".codex-doctor").join("history");
    if history_dir.exists() {
        fs::remove_dir_all(&history_dir).expect("remove history dir");
    }

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.load_history().expect("load history");

    assert!(app.history.is_empty());
    assert_eq!(app.selected_history, None);
    assert!(app.last_error.is_none());
}

#[test]
fn restore_selected_backup_without_selection_reports_error() {
    let codex_home = prepare_codex_home();
    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());

    let error = app
        .restore_selected_backup()
        .expect_err("restore should fail without selection");

    assert!(error.contains("No backup selected"));
}

#[test]
fn prune_backups_updates_backup_list_and_status() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute first repair");

    fs::write(codex_home.path().join("config.toml"), "").expect("clear config again");
    app.execute_repair().expect("execute second repair");

    app.load_backups().expect("load backups");
    assert!(app.backups.len() >= 2, "expected at least two backups");

    app.prune_backups(1).expect("prune backups");

    assert_eq!(app.backups.len(), 1);
    assert_eq!(app.selected_backup, None);
    assert!(app.status_message.contains("Pruned 1 backup(s)"));
    assert_eq!(app.last_operation_title.as_deref(), Some("Last prune"));
    assert!(app.last_operation_at.is_some());
    assert!(app.last_execution.is_empty());
}

#[test]
fn prune_backups_handles_missing_directory_gracefully() {
    let codex_home = prepare_codex_home();
    let backups_dir = codex_home.path().join(".codex-doctor-backups");
    if backups_dir.exists() {
        fs::remove_dir_all(&backups_dir).expect("remove backups dir");
    }

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.prune_backups(1).expect("prune backups");

    assert!(app.backups.is_empty());
    assert_eq!(app.selected_backup, None);
    assert!(app.status_message.contains("Pruned 0 backup(s)"));
    assert_eq!(app.last_operation_title.as_deref(), Some("Last prune"));
    assert!(app.last_operation_at.is_some());
}

#[test]
fn prune_backups_from_input_rejects_invalid_integer() {
    let codex_home = prepare_codex_home();
    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.backup_keep_latest_input = "abc".to_string();

    let error = app
        .prune_backups_from_input()
        .expect_err("invalid input should fail");

    assert!(error.contains("Keep latest must be a non-negative integer"));
}

#[test]
fn prune_backups_from_input_accepts_trimmed_integer() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute first repair");

    fs::write(codex_home.path().join("config.toml"), "").expect("clear config again");
    app.execute_repair().expect("execute second repair");

    app.load_backups().expect("load backups");
    assert!(app.backups.len() >= 2, "expected at least two backups");

    app.backup_keep_latest_input = " 1 ".to_string();
    app.prune_backups_from_input()
        .expect("trimmed integer should parse");

    assert_eq!(app.backups.len(), 1);
    assert!(app.status_message.contains("Pruned 1 backup(s)"));
}
