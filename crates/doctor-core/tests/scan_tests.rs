use std::fs;
use std::path::Path;

use doctor_core::scan_codex_home;
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

fn create_threads_table(connection: &Connection) {
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
    connection: &Connection,
    id: &str,
    rollout_path: &str,
    provider: &str,
    cwd: &str,
    archived_at: Option<i64>,
) {
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
                rollout_path,
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
fn scan_codex_home_returns_summary_and_provider_distribution() {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("scan")
        .join("sample-codex");
    let temp = tempdir().expect("tempdir");
    let codex_home = temp.path().join("sample-codex");
    copy_dir_recursive(&fixture_root, &codex_home);

    let db_path = codex_home.join("state_5.sqlite");
    let connection = Connection::open(&db_path).expect("open sqlite");
    create_threads_table(&connection);
    insert_thread(
        &connection,
        "thr_active",
        "/workspace/scan/active.jsonl",
        "openai",
        "/workspace/active",
        None,
    );
    insert_thread(
        &connection,
        "thr_archived",
        "/workspace/scan/archived.jsonl",
        "mirror",
        "/workspace/archived",
        Some(1_700_000_200_i64),
    );
    drop(connection);

    let report = scan_codex_home(&codex_home).expect("scan report");

    assert!(report.summary.config_present);
    assert!(report.summary.sqlite_present);
    assert!(report.summary.sqlite_readable);
    assert!(!report.summary.history_present);
    assert!(!report.summary.history_readable);


    assert_eq!(report.summary.root_provider.as_deref(), Some("openai"));
    assert_eq!(report.providers.rollout.get("openai"), Some(&1));
    assert_eq!(report.providers.rollout.get("mirror"), Some(&1));
    assert_eq!(report.providers.sqlite.get("openai"), Some(&1));
    assert_eq!(report.providers.sqlite.get("mirror"), Some(&1));
}
