use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::{params, Connection};
use serde_json::Value;
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

fn run_cli(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(args)
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse json output")
}

#[test]
fn scan_json_outputs_summary() {
    let codex_home = prepare_codex_home();

    let output = run_cli(&[
        "scan",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--json",
    ]);

    assert_eq!(output["summary"]["config_present"], true);
    assert_eq!(output["summary"]["sqlite_present"], true);
}

#[test]
fn diagnose_json_outputs_problems() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = run_cli(&[
        "diagnose",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--json",
    ]);

    assert_eq!(output["problems"][0]["code"], "missing_root_model_provider");
}

#[test]
fn repair_dry_run_json_outputs_execution_report() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--dry-run",
        "--json",
    ]);

    assert!(output["backup"].is_null());
    assert_eq!(output["applied"].as_array().expect("applied array").len(), 0);
    assert!(!output["skipped"].as_array().expect("skipped array").is_empty());
}

#[test]
fn backup_list_json_outputs_manifests() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let create_output = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args([
            "repair",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--backups-root",
            backups_root.path().to_str().expect("backups root path"),
            "--json",
        ])
        .output()
        .expect("run repair cli");
    assert!(
        create_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&create_output.stderr)
    );

    let output = run_cli(&[
        "backup",
        "list",
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);

    assert_eq!(output.as_array().expect("backup list array").len(), 1);
    assert_eq!(
        output[0]["source_codex_home"],
        codex_home.path().display().to_string()
    );
}
