use std::fs;
use std::path::PathBuf;

use doctor_core::{read_root_config_snapshot, RolloutRecord, ThreadLocation};
use tempfile::tempdir;

#[test]
fn parses_rollout_file_and_extracts_provider() {
    let dir = tempdir().expect("tempdir");
    let rollout_path = dir
        .path()
        .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000123.jsonl");

    let content = r#"{"timestamp":"2026-01-27T12:34:56Z","type":"session_meta","payload":{"id":"00000000-0000-0000-0000-000000000123","timestamp":"2026-01-27T12:34:56Z","cwd":"/workspace/demo","originator":"cli","cli_version":"0.0.0","source":"cli","model_provider":"openai"}}"#;
    fs::write(&rollout_path, format!("{content}\n")).expect("write rollout");

    let record =
        RolloutRecord::from_path(&rollout_path, ThreadLocation::Active).expect("parse rollout");

    assert_eq!(record.thread_id, "00000000-0000-0000-0000-000000000123");
    assert_eq!(record.session_meta.provider.as_deref(), Some("openai"));
    assert_eq!(record.session_meta.cwd, PathBuf::from("/workspace/demo"));
    assert_eq!(record.session_meta.timestamp, "2026-01-27T12:34:56Z");
    assert_eq!(record.location, ThreadLocation::Active);
    assert!(!record.archived);
}

#[test]
fn archived_location_is_represented_in_domain_enum() {
    let dir = tempdir().expect("tempdir");
    let archived_path = dir
        .path()
        .join("archived_sessions")
        .join("rollout-2026-01-27T12-34-56-00000000-0000-0000-0000-000000000456.jsonl");
    fs::create_dir_all(archived_path.parent().expect("parent")).expect("mkdirs");

    let content = r#"{"timestamp":"2026-01-27T12:34:56Z","type":"session_meta","payload":{"id":"00000000-0000-0000-0000-000000000456","timestamp":"2026-01-27T12:34:56Z","cwd":"/workspace/archive","originator":"cli","cli_version":"0.0.0","source":"cli","model_provider":"mirror"}}"#;
    fs::write(&archived_path, format!("{content}\n")).expect("write rollout");

    let record = RolloutRecord::from_path(&archived_path, ThreadLocation::Archived)
        .expect("parse archived rollout");

    assert_eq!(record.location, ThreadLocation::Archived);
    assert!(record.archived);
}

#[test]
fn reads_root_model_provider_from_config() {
    let dir = tempdir().expect("tempdir");
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "model_provider = \"openai\"\n").expect("write config");

    let snapshot = read_root_config_snapshot(&config_path).expect("parse config");

    assert_eq!(snapshot.model_provider.as_deref(), Some("openai"));
}
