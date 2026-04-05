use std::fs;
use std::path::{Path, PathBuf};

use doctor_core::{
    execute_repair_plan, list_backups, read_root_config_snapshot, read_thread_by_id, CodexLayout,
    RepairAction, RepairPlan,
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

const ACTIVE_THREAD_ID: &str = "00000000-0000-0000-0000-000000000123";
const ARCHIVED_THREAD_ID: &str = "00000000-0000-0000-0000-000000000456";
const ACTIVE_ROLLOUT_FILENAME: &str =
    "rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl";
const ARCHIVED_ROLLOUT_FILENAME: &str =
    "rollout-2026-01-26T09-00-00-00000000-0000-0000-0000-000000000456.jsonl";

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
    create_threads_table(&CodexLayout::from_codex_home(temp.path()).state_db);
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
fn upserts_missing_sqlite_thread_row_from_rollout_metadata() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());
    let plan = RepairPlan {
        actions: vec![RepairAction::UpsertSqliteThreadMetadata {
            thread_id: ACTIVE_THREAD_ID.to_string(),
        }],
    };

    let report = execute_repair_plan(codex_home.path(), backups_root.path(), &plan, false)
        .expect("execute repair plan");

    let thread = read_thread_by_id(&layout.state_db, ACTIVE_THREAD_ID)
        .expect("read sqlite row")
        .expect("sqlite row exists");
    assert_eq!(thread.model_provider, "openai");
    assert_eq!(thread.cwd, PathBuf::from("/workspace/active"));
    assert_eq!(
        thread.rollout_path,
        layout.sessions_dir.join(ACTIVE_ROLLOUT_FILENAME)
    );
    assert!(thread.archived_at.is_none());
    assert_eq!(report.applied.len(), 1);
    assert!(report.failed.is_empty());
    assert_eq!(
        list_backups(backups_root.path())
            .expect("list backups")
            .len(),
        1
    );
}

#[test]
fn rewrites_rollout_provider_metadata() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());
    let rollout_path = layout.sessions_dir.join(ACTIVE_ROLLOUT_FILENAME);
    let plan = RepairPlan {
        actions: vec![RepairAction::RewriteRolloutSessionMeta {
            thread_id: ACTIVE_THREAD_ID.to_string(),
            provider: "mirror".to_string(),
        }],
    };

    execute_repair_plan(codex_home.path(), backups_root.path(), &plan, false)
        .expect("execute repair plan");

    let content = fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(content.contains("\"model_provider\":\"mirror\""));
}

#[test]
fn moves_archived_rollout_back_to_sessions() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());
    let archived_path = layout.archived_sessions_dir.join(ARCHIVED_ROLLOUT_FILENAME);
    insert_thread(
        &layout.state_db,
        ARCHIVED_THREAD_ID,
        &archived_path,
        "mirror",
        "/workspace/archived",
        None,
    );
    let plan = RepairPlan {
        actions: vec![RepairAction::MoveRolloutToSessions {
            thread_id: ARCHIVED_THREAD_ID.to_string(),
        }],
    };

    execute_repair_plan(codex_home.path(), backups_root.path(), &plan, false)
        .expect("execute repair plan");

    assert!(!archived_path.exists());
    assert!(layout.sessions_dir.join(ARCHIVED_ROLLOUT_FILENAME).exists());
}

#[test]
fn patches_missing_root_provider_in_config() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());
    fs::write(&layout.config_toml, "").expect("clear config");
    let plan = RepairPlan {
        actions: vec![RepairAction::PatchConfigModelProvider {
            provider: "anthropic".to_string(),
        }],
    };

    execute_repair_plan(codex_home.path(), backups_root.path(), &plan, false)
        .expect("execute repair plan");

    let config = read_root_config_snapshot(&layout.config_toml).expect("read config snapshot");
    assert_eq!(config.model_provider.as_deref(), Some("anthropic"));
}

#[test]
fn dry_run_causes_zero_writes() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    let layout = CodexLayout::from_codex_home(codex_home.path());
    let rollout_path = layout.sessions_dir.join(ACTIVE_ROLLOUT_FILENAME);
    let original_rollout = fs::read_to_string(&rollout_path).expect("read rollout before");
    let original_config = fs::read_to_string(&layout.config_toml).expect("read config before");
    let plan = RepairPlan {
        actions: vec![
            RepairAction::UpsertSqliteThreadMetadata {
                thread_id: ACTIVE_THREAD_ID.to_string(),
            },
            RepairAction::RewriteRolloutSessionMeta {
                thread_id: ACTIVE_THREAD_ID.to_string(),
                provider: "mirror".to_string(),
            },
            RepairAction::PatchConfigModelProvider {
                provider: "mirror".to_string(),
            },
        ],
    };

    let report = execute_repair_plan(codex_home.path(), backups_root.path(), &plan, true)
        .expect("execute dry-run repair plan");

    assert_eq!(report.applied.len(), 0);
    assert_eq!(report.skipped.len(), 3);
    assert!(report.failed.is_empty());
    assert!(read_thread_by_id(&layout.state_db, ACTIVE_THREAD_ID)
        .expect("read sqlite row")
        .is_none());
    assert_eq!(
        fs::read_to_string(&rollout_path).expect("read rollout after"),
        original_rollout
    );
    assert_eq!(
        fs::read_to_string(&layout.config_toml).expect("read config after"),
        original_config
    );
    assert!(list_backups(backups_root.path())
        .expect("list backups")
        .is_empty());
}
