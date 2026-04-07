use std::collections::BTreeMap;
use std::path::PathBuf;

use doctor_core::{
    diagnose, ProblemCode, ProblemSeverity, ProviderDistribution, RolloutRecord,
    RolloutSessionMeta, RootConfigSnapshot, ScanReport, ScanSummary, SqliteThreadRecord,
    ThreadLocation,
};

fn base_report() -> ScanReport {
    ScanReport {
        summary: ScanSummary {
            config_present: true,
            sessions_present: true,
            sqlite_present: true,
            sqlite_readable: true,
            sqlite_locked: false,
            logs_present: true,
            logs_readable: true,
            history_present: true,
            history_readable: true,
            active_rollout_count: 1,
            archived_rollout_count: 0,
            locked_rollout_count: 0,
            root_provider: Some("openai".to_string()),
        },
        providers: ProviderDistribution {
            rollout: BTreeMap::new(),
            sqlite: BTreeMap::new(),
        },
        root_config: Some(RootConfigSnapshot {
            model_provider: Some("openai".to_string()),
        }),
        locked_rollout_paths: Vec::new(),
        rollout_records: vec![RolloutRecord {
            thread_id: "thr_123".to_string(),
            rollout_path: PathBuf::from("/tmp/sessions/rollout-123.jsonl"),
            session_meta: RolloutSessionMeta {
                provider: Some("openai".to_string()),
                cwd: PathBuf::from("/tmp/workspace"),
                timestamp: "2026-01-27T12:34:56Z".to_string(),
            },
            location: ThreadLocation::Active,
            archived: false,
        }],
        sqlite_threads: vec![SqliteThreadRecord {
            id: "thr_123".to_string(),
            rollout_path: PathBuf::from("/tmp/sessions/rollout-123.jsonl"),
            model_provider: "openai".to_string(),
            archived_at: None,
            cwd: PathBuf::from("/tmp/workspace"),
        }],
    }
}

#[test]
fn flags_missing_sqlite_thread_row() {
    let mut report = base_report();
    report.sqlite_threads.clear();

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingSqliteThreadRow));
}

#[test]
fn flags_stale_sqlite_rollout_path() {
    let mut report = base_report();
    report.sqlite_threads[0].rollout_path = PathBuf::from("/tmp/sessions/missing.jsonl");

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::StaleSqliteRolloutPath));
}

#[test]
fn flags_rollout_provider_mismatch() {
    let mut report = base_report();
    report.sqlite_threads[0].model_provider = "mirror".to_string();

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::RolloutProviderMismatch));
}

#[test]
fn flags_archived_state_mismatch() {
    let mut report = base_report();
    report.rollout_records[0].location = ThreadLocation::Archived;
    report.rollout_records[0].archived = true;
    report.summary.active_rollout_count = 0;
    report.summary.archived_rollout_count = 1;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::ArchivedStateMismatch));
}

#[test]
fn flags_resume_picker_provider_filtered() {
    let mut report = base_report();
    report.rollout_records[0].session_meta.provider = Some("anthropic".to_string());
    report.sqlite_threads[0].model_provider = "anthropic".to_string();

    let diagnosis = diagnose(&report);
    let problem = diagnosis
        .problems
        .iter()
        .find(|problem| problem.code == ProblemCode::ResumePickerProviderFiltered)
        .expect("provider filtered problem");

    assert!(problem
        .evidence
        .iter()
        .any(|line| line.contains("codex resume thr_123")));
    assert!(problem
        .suggested_fix_ids
        .contains(&"resume_by_thread_id".to_string()));
    assert!(problem
        .suggested_fix_ids
        .contains(&"switch_root_provider_for_resume".to_string()));
}

#[test]
fn flags_resume_picker_archived_filtered() {
    let mut report = base_report();
    report.rollout_records[0].location = ThreadLocation::Archived;
    report.rollout_records[0].archived = true;
    report.sqlite_threads[0].archived_at = Some(1_700_000_200);
    report.summary.active_rollout_count = 0;
    report.summary.archived_rollout_count = 1;

    let diagnosis = diagnose(&report);
    let problem = diagnosis
        .problems
        .iter()
        .find(|problem| problem.code == ProblemCode::ResumePickerArchivedFiltered)
        .expect("archived filtered problem");

    assert!(problem
        .evidence
        .iter()
        .any(|line| line.contains("codex resume thr_123")));
    assert!(problem
        .suggested_fix_ids
        .contains(&"resume_by_thread_id".to_string()));
    assert!(!diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::ArchivedStateMismatch));
}

#[test]
fn flags_missing_root_model_provider() {
    let mut report = base_report();
    report.summary.root_provider = None;
    report.root_config = Some(RootConfigSnapshot {
        model_provider: None,
    });

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingRootModelProvider));
}

#[test]
fn flags_missing_sessions_directory() {
    let mut report = base_report();
    report.summary.sessions_present = false;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingSessionsDirectory));
}

#[test]
fn flags_unreadable_sqlite_database() {
    let mut report = base_report();
    report.summary.sqlite_present = true;
    report.summary.sqlite_readable = false;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::UnreadableSqliteDatabase));
}

#[test]
fn flags_locked_database() {
    let mut report = base_report();
    report.summary.sqlite_locked = true;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::LockedDatabase));
}

#[test]
fn flags_missing_logs_sqlite() {
    let mut report = base_report();
    report.summary.logs_present = false;
    report.summary.logs_readable = false;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingLogsSqlite));
}

#[test]
fn flags_unreadable_logs_sqlite() {
    let mut report = base_report();
    report.summary.logs_present = true;
    report.summary.logs_readable = false;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::UnreadableLogsSqlite));
}

#[test]
fn flags_missing_history_jsonl() {
    let mut report = base_report();

    report.summary.history_present = false;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingHistoryJsonl));
}

#[test]
fn flags_unreadable_history_jsonl() {
    let mut report = base_report();
    report.summary.history_present = true;
    report.summary.history_readable = false;

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::UnreadableHistoryJsonl));
}

#[test]
fn flags_locked_rollout_file() {
    let mut report = base_report();
    report.summary.locked_rollout_count = 1;
    report.locked_rollout_paths = vec![PathBuf::from("/tmp/sessions/locked.jsonl")];

    let diagnosis = diagnose(&report);

    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::LockedRolloutFile));
}

#[test]
fn diagnosis_report_includes_evidence_and_fix_ids() {
    let mut report = base_report();
    report.sqlite_threads.clear();

    let diagnosis = diagnose(&report);
    let problem = diagnosis
        .problems
        .iter()
        .find(|problem| problem.code == ProblemCode::MissingSqliteThreadRow)
        .expect("problem exists");

    assert_eq!(problem.severity, ProblemSeverity::Warning);
    assert!(!problem.evidence.is_empty());
    assert!(!problem.suggested_fix_ids.is_empty());
}
