use std::collections::BTreeMap;
use std::path::PathBuf;

use doctor_core::{
    build_repair_plan, diagnose, ProblemCode, ProviderDistribution, RepairAction, RolloutRecord,
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
fn maps_stale_rollout_path_to_rebuild_action() {
    let mut report = base_report();
    report.sqlite_threads[0].rollout_path = PathBuf::from("/tmp/sessions/missing-rollout.jsonl");

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::StaleSqliteRolloutPath));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.iter().any(|action| matches!(
        action,
        RepairAction::RebuildMissingIndexFromRollout { thread_id, rollout_path }
            if thread_id == "thr_123" && rollout_path == &PathBuf::from("/tmp/sessions/rollout-123.jsonl")
    )));
}

#[test]
fn maps_missing_sqlite_row_to_upsert_action() {
    let mut report = base_report();
    report.sqlite_threads.clear();

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingSqliteThreadRow));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.iter().any(|action| matches!(
        action,
        RepairAction::UpsertSqliteThreadMetadata { thread_id }
            if thread_id == "thr_123"
    )));
}

#[test]
fn maps_archived_mismatch_to_move_action() {
    let mut report = base_report();
    report.rollout_records[0].location = ThreadLocation::Archived;
    report.rollout_records[0].archived = true;

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::ArchivedStateMismatch));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.iter().any(|action| matches!(
        action,
        RepairAction::MoveRolloutToSessions { thread_id }
            if thread_id == "thr_123"
    )));
}

#[test]
fn maps_provider_mismatch_to_rewrite_and_upsert_actions() {
    let mut report = base_report();
    report.rollout_records[0].session_meta.provider = Some("anthropic".to_string());

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::RolloutProviderMismatch));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.iter().any(|action| matches!(
        action,
        RepairAction::RewriteRolloutSessionMeta { thread_id, provider }
            if thread_id == "thr_123" && provider == "openai"
    )));
    assert!(plan.actions.iter().any(|action| matches!(
        action,
        RepairAction::UpsertSqliteThreadMetadata { thread_id }
            if thread_id == "thr_123"
    )));
}

#[test]
fn resume_picker_provider_filtered_is_report_only() {
    let mut report = base_report();
    report.rollout_records[0].session_meta.provider = Some("anthropic".to_string());
    report.sqlite_threads[0].model_provider = "anthropic".to_string();

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::ResumePickerProviderFiltered));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.is_empty());
}

#[test]
fn resume_picker_archived_filtered_is_report_only() {
    let mut report = base_report();
    report.rollout_records[0].location = ThreadLocation::Archived;
    report.rollout_records[0].archived = true;
    report.sqlite_threads[0].archived_at = Some(1_700_000_200);
    report.summary.active_rollout_count = 0;
    report.summary.archived_rollout_count = 1;

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::ResumePickerArchivedFiltered));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.is_empty());
}

#[test]
fn maps_missing_root_provider_to_patch_action_and_renders_summary() {
    let mut report = base_report();
    report.summary.root_provider = None;
    report.root_config = Some(RootConfigSnapshot {
        model_provider: None,
    });
    report.providers.rollout.insert("openai".to_string(), 1);

    let diagnosis = diagnose(&report);
    assert!(diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == ProblemCode::MissingRootModelProvider));

    let plan = build_repair_plan(&report, &diagnosis);

    assert!(plan.actions.iter().any(|action| matches!(
        action,
        RepairAction::PatchConfigModelProvider { provider }
            if provider == "openai"
    )));
    assert!(plan
        .render_dry_run_summary()
        .contains("patch_config_model_provider"));
}
