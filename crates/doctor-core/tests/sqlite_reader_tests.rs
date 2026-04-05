use std::path::PathBuf;

use doctor_core::{read_thread_by_id, read_threads, SqliteReaderError};
use rusqlite::{params, Connection};
use tempfile::tempdir;

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

fn insert_thread(connection: &Connection) {
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
                "thr_123",
                "/tmp/rollout-123.jsonl",
                1_700_000_000_i64,
                1_700_000_100_i64,
                "cli",
                "openai",
                "/tmp/workspace",
                "0.1.0",
                "Example thread",
                "read-only",
                "on-request",
                123_i64,
                "hello",
                1_700_000_200_i64,
            ],
        )
        .expect("insert thread");
}

#[test]
fn reads_threads_from_sqlite() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("state_5.sqlite");
    let connection = Connection::open(&db_path).expect("open sqlite");
    create_threads_table(&connection);
    insert_thread(&connection);
    drop(connection);

    let threads = read_threads(&db_path).expect("read threads");

    assert_eq!(threads.len(), 1);
    let thread = &threads[0];
    assert_eq!(thread.id, "thr_123");
    assert_eq!(thread.rollout_path, PathBuf::from("/tmp/rollout-123.jsonl"));
    assert_eq!(thread.model_provider, "openai");
    assert_eq!(thread.archived_at, Some(1_700_000_200));
    assert_eq!(thread.cwd, PathBuf::from("/tmp/workspace"));
}

#[test]
fn reads_single_thread_by_id() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("state_5.sqlite");
    let connection = Connection::open(&db_path).expect("open sqlite");
    create_threads_table(&connection);
    insert_thread(&connection);
    drop(connection);

    let thread = read_thread_by_id(&db_path, "thr_123")
        .expect("lookup by id")
        .expect("thread exists");

    assert_eq!(thread.id, "thr_123");
    assert_eq!(thread.model_provider, "openai");
}

#[test]
fn returns_open_error_for_missing_database() {
    let dir = tempdir().expect("tempdir");
    let missing_path = dir.path().join("missing.sqlite");

    let error = read_threads(&missing_path).expect_err("missing db should fail");

    assert!(matches!(error, SqliteReaderError::Open { .. }));
}
