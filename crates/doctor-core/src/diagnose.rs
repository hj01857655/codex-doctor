use std::collections::BTreeMap;

use crate::{ScanReport, ThreadLocation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemCode {
    MissingSqliteThreadRow,
    OrphanSqliteThreadRow,
    StaleSqliteRolloutPath,
    DuplicateRolloutThreadId,
    DuplicateActiveArchivedRollout,
    RolloutProviderMismatch,
    ArchivedStateMismatch,
    MissingRootModelProvider,
    MissingLogsSqlite,
    UnreadableLogsSqlite,
    MissingHistoryJsonl,
    UnreadableHistoryJsonl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosisProblem {
    pub code: ProblemCode,
    pub severity: ProblemSeverity,
    pub evidence: Vec<String>,
    pub suggested_fix_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosisReport {
    pub problems: Vec<DiagnosisProblem>,
}

pub fn diagnose(report: &ScanReport) -> DiagnosisReport {
    let sqlite_by_id: BTreeMap<_, _> = report
        .sqlite_threads
        .iter()
        .map(|thread| (thread.id.as_str(), thread))
        .collect();

    let rollout_by_path: BTreeMap<_, _> = report
        .rollout_records
        .iter()
        .map(|record| (record.rollout_path.as_path(), record))
        .collect();
    let mut thread_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut thread_locations: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
    for record in &report.rollout_records {
        *thread_counts.entry(record.thread_id.as_str()).or_insert(0) += 1;
        let entry = thread_locations
            .entry(record.thread_id.as_str())
            .or_insert((false, false));
        if matches!(record.location, ThreadLocation::Archived) || record.archived {
            entry.1 = true;
        } else {
            entry.0 = true;
        }
    }

    let mut problems = Vec::new();

    if report.summary.root_provider.is_none() {
        problems.push(DiagnosisProblem {
            code: ProblemCode::MissingRootModelProvider,
            severity: ProblemSeverity::Warning,
            evidence: vec!["root model_provider is missing from config/summary".to_string()],
            suggested_fix_ids: vec!["patch_config_model_provider".to_string()],
        });
    }

    for sqlite_row in &report.sqlite_threads {
        if !report
            .rollout_records
            .iter()
            .any(|record| record.thread_id == sqlite_row.id)
        {
            problems.push(DiagnosisProblem {
                code: ProblemCode::OrphanSqliteThreadRow,
                severity: ProblemSeverity::Warning,
                evidence: vec![format!(
                    "sqlite row {} has no matching rollout record",
                    sqlite_row.id
                )],
                suggested_fix_ids: Vec::new(),
            });
        }
    }

    for (thread_id, count) in &thread_counts {
        if *count > 1 {
            problems.push(DiagnosisProblem {
                code: ProblemCode::DuplicateRolloutThreadId,
                severity: ProblemSeverity::Warning,
                evidence: vec![format!(
                    "thread {} appears in {} rollout records",
                    thread_id, count
                )],
                suggested_fix_ids: Vec::new(),
            });
        }
    }

    for (thread_id, (has_active, has_archived)) in &thread_locations {
        if *has_active && *has_archived {
            problems.push(DiagnosisProblem {
                code: ProblemCode::DuplicateActiveArchivedRollout,
                severity: ProblemSeverity::Warning,
                evidence: vec![format!(
                    "thread {} appears in both active and archived rollout locations",
                    thread_id
                )],
                suggested_fix_ids: Vec::new(),
            });
        }
    }

    if !report.summary.logs_present {
        problems.push(DiagnosisProblem {
            code: ProblemCode::MissingLogsSqlite,
            severity: ProblemSeverity::Info,
            evidence: vec!["logs_2.sqlite is missing from sqlite home".to_string()],
            suggested_fix_ids: Vec::new(),
        });
    } else if !report.summary.logs_readable {
        problems.push(DiagnosisProblem {
            code: ProblemCode::UnreadableLogsSqlite,
            severity: ProblemSeverity::Warning,
            evidence: vec!["logs_2.sqlite exists but could not be inspected".to_string()],
            suggested_fix_ids: Vec::new(),
        });
    }

    if !report.summary.history_present {
        problems.push(DiagnosisProblem {
            code: ProblemCode::MissingHistoryJsonl,
            severity: ProblemSeverity::Info,
            evidence: vec!["history.jsonl is missing from codex home".to_string()],
            suggested_fix_ids: Vec::new(),
        });
    } else if !report.summary.history_readable {
        problems.push(DiagnosisProblem {
            code: ProblemCode::UnreadableHistoryJsonl,
            severity: ProblemSeverity::Warning,
            evidence: vec!["history.jsonl exists but could not be read".to_string()],
            suggested_fix_ids: Vec::new(),
        });
    }

    for rollout in &report.rollout_records {
        match sqlite_by_id.get(rollout.thread_id.as_str()) {
            None => problems.push(DiagnosisProblem {
                code: ProblemCode::MissingSqliteThreadRow,
                severity: ProblemSeverity::Warning,
                evidence: vec![format!(
                    "rollout {} has no matching sqlite thread row",
                    rollout.thread_id
                )],
                suggested_fix_ids: vec!["upsert_sqlite_thread_metadata".to_string()],
            }),
            Some(sqlite_row) => {
                if sqlite_row.rollout_path != rollout.rollout_path {
                    let stale_path =
                        !rollout_by_path.contains_key(sqlite_row.rollout_path.as_path());
                    if stale_path {
                        problems.push(DiagnosisProblem {
                            code: ProblemCode::StaleSqliteRolloutPath,
                            severity: ProblemSeverity::Warning,
                            evidence: vec![format!(
                                "sqlite row {} points to missing rollout path {}",
                                sqlite_row.id,
                                sqlite_row.rollout_path.display()
                            )],
                            suggested_fix_ids: vec![
                                "rebuild_missing_index_from_rollout".to_string()
                            ],
                        });
                    }
                }

                if rollout.session_meta.provider.as_deref()
                    != Some(sqlite_row.model_provider.as_str())
                {
                    problems.push(DiagnosisProblem {
                        code: ProblemCode::RolloutProviderMismatch,
                        severity: ProblemSeverity::Warning,
                        evidence: vec![format!(
                            "rollout {} provider {:?} != sqlite provider {}",
                            rollout.thread_id,
                            rollout.session_meta.provider,
                            sqlite_row.model_provider
                        )],
                        suggested_fix_ids: vec![
                            "rewrite_rollout_session_meta".to_string(),
                            "upsert_sqlite_thread_metadata".to_string(),
                        ],
                    });
                }

                let sqlite_archived = sqlite_row.archived_at.is_some();
                let rollout_archived =
                    matches!(rollout.location, ThreadLocation::Archived) || rollout.archived;
                if sqlite_archived != rollout_archived {
                    problems.push(DiagnosisProblem {
                        code: ProblemCode::ArchivedStateMismatch,
                        severity: ProblemSeverity::Warning,
                        evidence: vec![format!(
                            "thread {} archived state mismatch between rollout ({}) and sqlite ({})",
                            rollout.thread_id, rollout_archived, sqlite_archived
                        )],
                        suggested_fix_ids: vec![
                            "move_rollout_to_archive".to_string(),
                            "move_rollout_to_sessions".to_string(),
                        ],
                    });
                }
            }
        }
    }

    DiagnosisReport { problems }
}
