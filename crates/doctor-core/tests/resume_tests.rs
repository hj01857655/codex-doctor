use std::collections::BTreeMap;
use std::path::PathBuf;

use doctor_core::{
    best_resume_candidate_for_current_cwd, build_resume_doctor_report, ProviderDistribution,
    RolloutRecord, RolloutSessionMeta, RootConfigSnapshot, ScanReport, ScanSummary,
    SqliteThreadRecord, ThreadLocation,
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
                cwd: PathBuf::from("/workspace/active"),
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
            cwd: PathBuf::from("/workspace/active"),
        }],
    }
}

#[test]
fn visible_candidate_has_direct_resume_command() {
    let report = base_report();

    let resume = build_resume_doctor_report(&report, &PathBuf::from("/workspace/active"));
    let candidate = &resume.candidates[0];

    assert!(candidate.default_picker_visible);
    assert!(candidate.blockers.is_empty());
    assert_eq!(
        candidate.direct_resume_command.as_deref(),
        Some("codex resume thr_123")
    );
}

#[test]
fn provider_mismatch_marks_candidate_hidden() {
    let mut report = base_report();
    report.rollout_records[0].session_meta.provider = Some("anthropic".to_string());
    report.sqlite_threads[0].model_provider = "anthropic".to_string();

    let resume = build_resume_doctor_report(&report, &PathBuf::from("/workspace/active"));
    let candidate = &resume.candidates[0];

    assert!(!candidate.default_picker_visible);
    assert!(candidate.blockers.iter().any(|blocker| matches!(
        blocker,
        doctor_core::ResumeBlocker::ProviderMismatch { session_provider, current_provider }
            if session_provider == "anthropic" && current_provider == "openai"
    )));
}

#[test]
fn cwd_mismatch_marks_candidate_hidden() {
    let report = base_report();

    let resume = build_resume_doctor_report(&report, &PathBuf::from("/workspace/other"));
    let candidate = &resume.candidates[0];

    assert!(!candidate.default_picker_visible);
    assert!(candidate
        .blockers
        .iter()
        .any(|blocker| matches!(blocker, doctor_core::ResumeBlocker::CwdMismatch { .. })));
}

#[test]
fn missing_sqlite_row_removes_direct_resume_command() {
    let mut report = base_report();
    report.sqlite_threads.clear();

    let resume = build_resume_doctor_report(&report, &PathBuf::from("/workspace/active"));
    let candidate = &resume.candidates[0];

    assert!(!candidate.default_picker_visible);
    assert!(candidate
        .blockers
        .iter()
        .any(|blocker| matches!(blocker, doctor_core::ResumeBlocker::MissingSqliteThreadRow)));
    assert!(candidate.direct_resume_command.is_none());
}

#[test]
fn best_resume_candidate_prefers_latest_match_in_current_cwd() {
    let mut report = base_report();
    report.rollout_records.push(RolloutRecord {
        thread_id: "thr_456".to_string(),
        rollout_path: PathBuf::from("/tmp/sessions/rollout-456.jsonl"),
        session_meta: RolloutSessionMeta {
            provider: Some("openai".to_string()),
            cwd: PathBuf::from("/workspace/active"),
            timestamp: "2026-01-27T13:34:56Z".to_string(),
        },
        location: ThreadLocation::Active,
        archived: false,
    });
    report.sqlite_threads.push(SqliteThreadRecord {
        id: "thr_456".to_string(),
        rollout_path: PathBuf::from("/tmp/sessions/rollout-456.jsonl"),
        model_provider: "openai".to_string(),
        archived_at: None,
        cwd: PathBuf::from("/workspace/active"),
    });

    let resume = build_resume_doctor_report(&report, &PathBuf::from("/workspace/active"));
    let best = best_resume_candidate_for_current_cwd(&resume).expect("best candidate");

    assert_eq!(best.thread_id, "thr_456");
}
