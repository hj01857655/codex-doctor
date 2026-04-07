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

fn add_second_rollout_in_other_cwd(codex_home: &Path, thread_id: &str, cwd: &str) {
    let rollout_path = codex_home
        .join("sessions")
        .join(format!("rollout-2026-01-27T13-34-56-{thread_id}.jsonl"));
    let content = format!(
        "{{\"timestamp\":\"2026-01-27T13:34:56Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{thread_id}\",\"timestamp\":\"2026-01-27T13:34:56Z\",\"cwd\":\"{cwd}\",\"originator\":\"cli\",\"cli_version\":\"0.0.0\",\"source\":\"cli\",\"model_provider\":\"openai\"}}}}\n"
    );
    fs::write(&rollout_path, content).expect("write extra rollout");
    insert_thread(
        &codex_home.join("state_5.sqlite"),
        thread_id,
        &rollout_path,
        "openai",
        cwd,
        None,
    );
}

fn prepare_default_codex_home_root() -> tempfile::TempDir {
    let root = tempdir().expect("create tempdir");
    let codex_home = root.path().join(".codex");
    fs::create_dir_all(&codex_home).expect("create .codex directory");

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("backup")
        .join("sample-codex");

    copy_dir_recursive(&source, &codex_home);
    create_threads_table(&codex_home.join("state_5.sqlite"));
    insert_thread(
        &codex_home.join("state_5.sqlite"),
        "00000000-0000-0000-0000-000000000123",
        &codex_home
            .join("sessions")
            .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl"),
        "openai",
        "/workspace/active",
        None,
    );
    root
}

fn prepare_codex_home_with_separate_sqlite_home() -> (tempfile::TempDir, tempfile::TempDir) {
    let codex_home = prepare_codex_home();
    let sqlite_home = tempdir().expect("create sqlite home");
    fs::rename(
        codex_home.path().join("state_5.sqlite"),
        sqlite_home.path().join("state_5.sqlite"),
    )
    .expect("move sqlite database");
    (codex_home, sqlite_home)
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

fn update_thread_provider(path: &Path, id: &str, provider: &str, archived_at: Option<i64>) {
    let connection = Connection::open(path).expect("open sqlite");
    connection
        .execute(
            "UPDATE threads SET model_provider = ?1, archived_at = ?2 WHERE id = ?3",
            params![provider, archived_at, id],
        )
        .expect("update thread provider");
}

fn rewrite_rollout_provider(path: &Path, provider: &str) {
    let content = fs::read_to_string(path).expect("read rollout");
    let updated = content.replace(
        "\"model_provider\":\"openai\"",
        &format!("\"model_provider\":\"{provider}\""),
    );
    fs::write(path, updated).expect("write rollout");
}

fn write_fake_codex_script(path: &Path, log_path: &Path) {
    #[cfg(windows)]
    let script = format!(
        "@echo off\r\necho %* > \"{}\"\r\nexit /b 0\r\n",
        log_path.display()
    );

    #[cfg(not(windows))]
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"{}\"\n",
        log_path.display()
    );

    #[cfg(windows)]
    fs::write(path, script).expect("write fake codex");

    #[cfg(not(windows))]
    std::fs::write(path, script.as_bytes()).expect("write fake codex");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod fake codex");
    }
}

fn run_cli(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_codex-doctor"))
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

fn run_cli_text(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_codex-doctor"))
        .args(args)
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("parse stdout as utf8")
}

fn run_cli_text_with_input_and_env(args: &[&str], input: &str, envs: &[(&str, &str)]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codex-doctor"));
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = command.spawn().expect("spawn cli");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input.as_bytes()).expect("write cli stdin");
    }
    let output = child.wait_with_output().expect("wait cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("parse stdout as utf8")
}

fn run_cli_with_env(args: &[&str], envs: &[(&str, &str)]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codex-doctor"));
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("run cli");

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
    assert_eq!(output["summary"]["sessions_present"], true);
    assert_eq!(output["summary"]["sqlite_present"], true);
    assert_eq!(output["summary"]["sqlite_locked"], false);
    assert_eq!(output["summary"]["logs_present"], false);
    assert_eq!(output["summary"]["logs_readable"], false);
    assert_eq!(output["summary"]["history_present"], true);
    assert_eq!(output["summary"]["history_readable"], true);
}

#[test]
fn scan_json_respects_sqlite_home_override() {
    let (codex_home, sqlite_home) = prepare_codex_home_with_separate_sqlite_home();

    let output = run_cli(&[
        "scan",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--sqlite-home",
        sqlite_home.path().to_str().expect("sqlite home path"),
        "--json",
    ]);

    assert_eq!(output["summary"]["sqlite_present"], true);
    assert_eq!(output["summary"]["sqlite_readable"], true);
    assert_eq!(
        output["sqlite_threads"]
            .as_array()
            .expect("sqlite threads")
            .len(),
        1
    );
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

    let codes = output["problems"]
        .as_array()
        .expect("problems array")
        .iter()
        .map(|problem| problem["code"].as_str().expect("problem code"))
        .collect::<Vec<_>>();

    assert!(codes.contains(&"missing_root_model_provider"));
    assert!(codes.contains(&"missing_logs_sqlite"));
    assert!(!codes.contains(&"missing_history_jsonl"));
}

#[test]
fn diagnose_without_json_outputs_human_readable_report() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = run_cli_text(&[
        "diagnose",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
    ]);

    assert!(output.contains("Codex Doctor - Diagnosis Report"));
    assert!(output.contains("Found"));
    assert!(output.contains("MissingRootModelProvider"));
}

#[test]
fn diagnose_json_reports_resume_picker_provider_filtered() {
    let codex_home = prepare_codex_home();
    let rollout_path = codex_home
        .path()
        .join("sessions")
        .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl");
    rewrite_rollout_provider(&rollout_path, "anthropic");
    update_thread_provider(
        &codex_home.path().join("state_5.sqlite"),
        "00000000-0000-0000-0000-000000000123",
        "anthropic",
        None,
    );

    let output = run_cli(&[
        "diagnose",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--json",
    ]);

    let codes = output["problems"]
        .as_array()
        .expect("problems array")
        .iter()
        .map(|problem| problem["code"].as_str().expect("problem code"))
        .collect::<Vec<_>>();

    assert!(codes.contains(&"resume_picker_provider_filtered"));
}

#[test]
fn diagnose_text_reports_direct_resume_recovery_guidance() {
    let codex_home = prepare_codex_home();
    let rollout_path = codex_home
        .path()
        .join("sessions")
        .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl");
    rewrite_rollout_provider(&rollout_path, "anthropic");
    update_thread_provider(
        &codex_home.path().join("state_5.sqlite"),
        "00000000-0000-0000-0000-000000000123",
        "anthropic",
        None,
    );

    let output = run_cli_text(&[
        "diagnose",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
    ]);

    assert!(output.contains("ResumePickerProviderFiltered"));
    assert!(output.contains("codex resume 00000000-0000-0000-0000-000000000123"));
    assert!(output.contains("switch_root_provider_for_resume"));
}

#[test]
fn resume_doctor_json_reports_hidden_candidate_and_command() {
    let codex_home = prepare_codex_home();
    let rollout_path = codex_home
        .path()
        .join("sessions")
        .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl");
    rewrite_rollout_provider(&rollout_path, "anthropic");
    update_thread_provider(
        &codex_home.path().join("state_5.sqlite"),
        "00000000-0000-0000-0000-000000000123",
        "anthropic",
        None,
    );

    let output = run_cli(&[
        "resume-doctor",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--current-cwd",
        "/workspace/active",
        "--all",
        "--json",
    ]);

    let candidate = &output["candidates"][0];
    assert_eq!(
        candidate["direct_resume_command"],
        "codex resume 00000000-0000-0000-0000-000000000123"
    );
    assert_eq!(candidate["default_picker_visible"], false);
    assert!(candidate["blockers"]
        .as_array()
        .expect("blockers")
        .iter()
        .any(|item| item["type"] == "provider_mismatch"));
}

#[test]
fn resume_doctor_text_reports_visibility_and_blockers() {
    let codex_home = prepare_codex_home();

    let output = run_cli_text_with_input_and_env(
        &[
            "resume-doctor",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--current-cwd",
            "/workspace/other",
        ],
        "\n",
        &[],
    );

    assert!(output.contains("Codex Doctor - Resume Doctor"));
    assert!(output.contains("No default-visible current-cwd sessions found"));
    assert!(output.contains("Use --all"));
}

#[test]
fn resume_doctor_defaults_to_current_cwd_only() {
    let codex_home = prepare_codex_home();
    add_second_rollout_in_other_cwd(
        codex_home.path(),
        "00000000-0000-0000-0000-000000000456",
        "/workspace/other",
    );

    let output = run_cli(&[
        "resume-doctor",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--current-cwd",
        "/workspace/active",
        "--json",
    ]);

    assert_eq!(
        output["candidates"].as_array().expect("candidates").len(),
        1
    );
    assert_eq!(
        output["candidates"][0]["thread_id"],
        "00000000-0000-0000-0000-000000000123"
    );
}

#[test]
fn resume_doctor_all_includes_other_cwds_sorted_newest_first() {
    let codex_home = prepare_codex_home();
    add_second_rollout_in_other_cwd(
        codex_home.path(),
        "00000000-0000-0000-0000-000000000456",
        "/workspace/other",
    );

    let output = run_cli(&[
        "resume-doctor",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--current-cwd",
        "/workspace/active",
        "--all",
        "--json",
    ]);

    let candidates = output["candidates"].as_array().expect("candidates");
    assert_eq!(candidates.len(), 3);
    assert_eq!(
        candidates[0]["thread_id"],
        "00000000-0000-0000-0000-000000000456"
    );
    assert_eq!(
        candidates[1]["thread_id"],
        "00000000-0000-0000-0000-000000000123"
    );
}

#[test]
fn resume_doctor_uses_default_codex_home_without_explicit_path() {
    let root = prepare_default_codex_home_root();

    let output = run_cli_with_env(
        &[
            "resume-doctor",
            "--current-cwd",
            "/workspace/active",
            "--json",
        ],
        &[
            (
                "USERPROFILE",
                root.path().to_str().expect("userprofile path"),
            ),
            ("HOME", root.path().to_str().expect("home path")),
        ],
    );

    assert_eq!(
        output["candidates"][0]["direct_resume_command"],
        "codex resume 00000000-0000-0000-0000-000000000123"
    );
    assert_eq!(output["candidates"][0]["default_picker_visible"], true);
}

#[test]
fn resume_doctor_executes_selected_resume_command() {
    let codex_home = prepare_codex_home();
    let temp = tempdir().expect("create exec tempdir");
    let log_path = temp.path().join("resume.log");
    let script_path = temp.path().join("fake-codex.cmd");
    write_fake_codex_script(&script_path, &log_path);

    let output = run_cli_text_with_input_and_env(
        &[
            "resume-doctor",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--current-cwd",
            "/workspace/active",
        ],
        "1\n",
        &[(
            "CODEX_DOCTOR_CODEX_BIN",
            script_path.to_str().expect("script path"),
        )],
    );

    let logged = fs::read_to_string(&log_path).expect("read log");
    assert!(output.contains("Select session number"));
    assert!(output.contains("Running: codex resume 00000000-0000-0000-0000-000000000123"));
    assert!(logged.contains("resume 00000000-0000-0000-0000-000000000123"));
}

#[test]
fn resume_doctor_enter_cancels_without_running_resume() {
    let codex_home = prepare_codex_home();
    let temp = tempdir().expect("create exec tempdir");
    let log_path = temp.path().join("resume-cancel.log");
    let script_path = temp.path().join("fake-codex.cmd");
    write_fake_codex_script(&script_path, &log_path);

    let output = run_cli_text_with_input_and_env(
        &[
            "resume-doctor",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--current-cwd",
            "/workspace/active",
        ],
        "\n",
        &[(
            "CODEX_DOCTOR_CODEX_BIN",
            script_path.to_str().expect("script path"),
        )],
    );

    assert!(output.contains("Select session number"));
    assert!(!output.contains("Running: codex resume"));
    assert!(!log_path.exists());
}

#[test]
fn resume_doctor_finds_codex_cmd_from_userprofile_without_path_entry() {
    let codex_home = prepare_codex_home();
    let temp = tempdir().expect("create exec tempdir");
    let profile_root = temp.path().join("profile");
    let npm_dir = profile_root.join("AppData").join("Roaming").join("npm");
    fs::create_dir_all(&npm_dir).expect("create npm dir");
    let log_path = temp.path().join("resume-profile.log");
    let script_path = npm_dir.join("codex.cmd");
    write_fake_codex_script(&script_path, &log_path);

    let output = run_cli_text_with_input_and_env(
        &[
            "resume-doctor",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--current-cwd",
            "/workspace/active",
        ],
        "1\n",
        &[
            ("USERPROFILE", profile_root.to_str().expect("profile path")),
            (
                "APPDATA",
                profile_root
                    .join("AppData")
                    .join("Roaming")
                    .to_str()
                    .expect("appdata"),
            ),
            ("PATH", "C:\\Windows\\System32"),
        ],
    );

    let logged = fs::read_to_string(&log_path).expect("read log");
    assert!(output.contains("Running: codex resume 00000000-0000-0000-0000-000000000123"));
    assert!(logged.contains("resume 00000000-0000-0000-0000-000000000123"));
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
    assert_eq!(
        output["applied"].as_array().expect("applied array").len(),
        0
    );
    assert!(!output["skipped"]
        .as_array()
        .expect("skipped array")
        .is_empty());
}

#[test]
fn repair_json_respects_sqlite_home_override() {
    let (codex_home, sqlite_home) = prepare_codex_home_with_separate_sqlite_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--sqlite-home",
        sqlite_home.path().to_str().expect("sqlite home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);

    assert_eq!(
        output["backup"]["manifest"]["source_codex_home"],
        codex_home.path().display().to_string()
    );
    assert!(output["applied"]
        .as_array()
        .expect("applied array")
        .iter()
        .any(|entry| entry["action"]["type"] == "patch_config_model_provider"));
    assert!(sqlite_home.path().join("state_5.sqlite").exists());
}

#[test]
fn repair_without_json_outputs_human_readable_report() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = run_cli_text(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
    ]);

    assert!(output.contains("Codex Doctor - Repair Execution"));
    assert!(output.contains("Backup created:"));
    assert!(output.contains("Applied:"));
}

#[test]
fn backup_list_json_outputs_manifests() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let create_output = Command::new(env!("CARGO_BIN_EXE_codex-doctor"))
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

#[test]
fn backup_list_without_json_outputs_human_readable_report() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let _ = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);

    let output = run_cli_text(&[
        "backup",
        "list",
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
    ]);

    assert!(output.contains("Codex Doctor - Backup List"));
    assert!(output.contains("Found 1 backup"));
    assert!(output.contains("Backup ID:"));
}

#[test]
fn repair_with_save_history_writes_history_entry_json() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--save-history",
        "--json",
    ]);

    let applied_len = output["applied"].as_array().expect("applied array").len();
    assert!(applied_len >= 1, "expected at least one applied action");

    let history_dir = codex_home.path().join(".codex-doctor").join("history");
    let entries: Vec<_> = fs::read_dir(&history_dir)
        .expect("read history dir")
        .collect();
    assert_eq!(entries.len(), 1);

    let history_json = fs::read_to_string(entries[0].as_ref().expect("history dir entry").path())
        .expect("read history json");
    let history_value: Value = serde_json::from_str(&history_json).expect("parse history json");
    assert_eq!(
        history_value["codex_home"],
        codex_home.path().display().to_string()
    );
    assert_eq!(history_value["actions_applied"], applied_len);
}

#[test]
fn history_json_outputs_saved_entries() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let repair_output = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--save-history",
        "--json",
    ]);

    let history_dir = codex_home.path().join(".codex-doctor").join("history");
    let output = run_cli(&[
        "history",
        "--history-dir",
        history_dir.to_str().expect("history dir path"),
        "--json",
    ]);

    let entries = output.as_array().expect("history entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["codex_home"],
        codex_home.path().display().to_string()
    );
    assert_eq!(
        entries[0]["actions_applied"],
        repair_output["applied"]
            .as_array()
            .expect("applied array")
            .len()
    );
}

#[test]
fn scan_without_json_outputs_human_readable_report() {
    let codex_home = prepare_codex_home();

    let output = run_cli_text(&[
        "scan",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
    ]);

    assert!(output.contains("Codex Doctor - Scan Report"));
    assert!(output.contains("Summary:"));
    assert!(output.contains("Logs present:"));
    assert!(output.contains("History present:"));
    assert!(output.contains("Active sessions:"));
}

#[test]
fn history_without_json_outputs_human_readable_report() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let _ = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--save-history",
        "--json",
    ]);

    let history_dir = codex_home.path().join(".codex-doctor").join("history");
    let output = run_cli_text(&[
        "history",
        "--history-dir",
        history_dir.to_str().expect("history dir path"),
    ]);

    assert!(output.contains("Codex Doctor - Repair History"));
    assert!(output.contains("Codex home:"));
    assert!(output.contains("Actions:"));
}

#[test]
fn backup_restore_without_json_outputs_success_message() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");

    let repair_output = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);

    let snapshot_dir = repair_output["backup"]["snapshot_dir"]
        .as_str()
        .expect("snapshot dir");

    let output_text = run_cli_text(&[
        "backup",
        "restore",
        "--snapshot-dir",
        snapshot_dir,
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
    ]);

    assert!(output_text.contains("✅ Backup restored successfully"));
}

#[test]
fn backup_prune_without_json_outputs_summary() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");

    for _ in 0..2 {
        fs::write(codex_home.path().join("config.toml"), "").expect("clear config");
        run_cli(&[
            "repair",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--backups-root",
            backups_root.path().to_str().expect("backups root path"),
            "--json",
        ]);
    }

    let output_text = run_cli_text(&[
        "backup",
        "prune",
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--keep-latest",
        "1",
    ]);

    assert!(output_text.contains("🗑️  Pruned"));
    assert!(output_text.contains("backup(s)"));
}

#[test]
fn backup_restore_json_restores_previous_config_state() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let repair_output = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);

    let snapshot_dir = repair_output["backup"]["snapshot_dir"]
        .as_str()
        .expect("snapshot dir");

    fs::write(
        codex_home.path().join("config.toml"),
        "model_provider = \"broken\"\n",
    )
    .expect("mutate config");

    let restore_output = run_cli(&[
        "backup",
        "restore",
        "--snapshot-dir",
        snapshot_dir,
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--json",
    ]);

    assert_eq!(restore_output["restored"], true);
    let restored = fs::read_to_string(codex_home.path().join("config.toml")).expect("read config");
    assert!(!restored.contains("model_provider = \"broken\""));

    let diagnosis_output = run_cli(&[
        "diagnose",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--json",
    ]);
    assert!(diagnosis_output["problems"]
        .as_array()
        .expect("problems array")
        .iter()
        .any(|problem| problem["code"] == "missing_root_model_provider"));
}

#[test]
fn backup_prune_json_removes_older_snapshots() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");

    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");
    let first_repair = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);
    let first_backup_id = first_repair["backup"]["backup_id"]
        .as_str()
        .expect("first backup id")
        .to_string();

    fs::write(codex_home.path().join("config.toml"), "").expect("clear config again");
    let second_repair = run_cli(&[
        "repair",
        "--codex-home",
        codex_home.path().to_str().expect("codex home path"),
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);
    let second_backup_id = second_repair["backup"]["backup_id"]
        .as_str()
        .expect("second backup id")
        .to_string();

    let prune_output = run_cli(&[
        "backup",
        "prune",
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--keep-latest",
        "1",
        "--json",
    ]);

    let removed = prune_output["removed_backup_ids"]
        .as_array()
        .expect("removed backup ids");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], first_backup_id);

    let backups = run_cli(&[
        "backup",
        "list",
        "--backups-root",
        backups_root.path().to_str().expect("backups root path"),
        "--json",
    ]);
    let manifests = backups.as_array().expect("backup manifests");
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0]["backup_id"], second_backup_id);
}
