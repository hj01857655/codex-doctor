use std::path::PathBuf;

use doctor_core::CodexLayout;

#[test]
fn resolves_default_layout_from_codex_home() {
    let layout = CodexLayout::from_codex_home("/tmp/example/.codex");

    assert!(layout.config_toml.ends_with("config.toml"));
    assert!(layout.sessions_dir.ends_with("sessions"));
    assert!(layout.archived_sessions_dir.ends_with("archived_sessions"));
    assert!(layout.state_db.ends_with("state_5.sqlite"));
    assert!(layout.logs_db.ends_with("logs_2.sqlite"));
    assert!(layout.history_jsonl.ends_with("history.jsonl"));
    assert_eq!(layout.sqlite_home, PathBuf::from("/tmp/example/.codex"));
}

#[test]
fn respects_sqlite_home_override() {
    let layout = CodexLayout::from_codex_home_and_env(
        "/tmp/example/.codex",
        Some(PathBuf::from("/tmp/sqlite-home")),
    );

    assert_eq!(layout.sqlite_home, PathBuf::from("/tmp/sqlite-home"));
    assert_eq!(layout.state_db, PathBuf::from("/tmp/sqlite-home/state_5.sqlite"));
    assert_eq!(layout.logs_db, PathBuf::from("/tmp/sqlite-home/logs_2.sqlite"));
}
