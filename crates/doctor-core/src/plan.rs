use crate::{DiagnosisReport, ProblemCode, ScanReport, ThreadLocation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairAction {
    RebuildMissingIndexFromRollout {
        thread_id: String,
        rollout_path: std::path::PathBuf,
    },
    UpsertSqliteThreadMetadata {
        thread_id: String,
    },
    MoveRolloutToArchive {
        thread_id: String,
    },
    MoveRolloutToSessions {
        thread_id: String,
    },
    RewriteRolloutSessionMeta {
        thread_id: String,
        provider: String,
    },
    PatchConfigModelProvider {
        provider: String,
    },
}

impl RepairAction {
    fn action_id(&self) -> &'static str {
        match self {
            Self::RebuildMissingIndexFromRollout { .. } => "rebuild_missing_index_from_rollout",
            Self::UpsertSqliteThreadMetadata { .. } => "upsert_sqlite_thread_metadata",
            Self::MoveRolloutToArchive { .. } => "move_rollout_to_archive",
            Self::MoveRolloutToSessions { .. } => "move_rollout_to_sessions",
            Self::RewriteRolloutSessionMeta { .. } => "rewrite_rollout_session_meta",
            Self::PatchConfigModelProvider { .. } => "patch_config_model_provider",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepairPlan {
    pub actions: Vec<RepairAction>,
}

impl RepairPlan {
    pub fn render_dry_run_summary(&self) -> String {
        if self.actions.is_empty() {
            return "No repair actions required.".to_string();
        }

        let mut summary = String::from("Planned repair actions:\n");
        for action in &self.actions {
            summary.push_str("- ");
            summary.push_str(action.action_id());
            summary.push('\n');
        }
        summary.trim_end().to_string()
    }

    fn push_unique(&mut self, action: RepairAction) {
        if !self.actions.contains(&action) {
            self.actions.push(action);
        }
    }
}

pub fn build_repair_plan(report: &ScanReport, diagnosis: &DiagnosisReport) -> RepairPlan {
    let mut plan = RepairPlan::default();

    if has_problem(diagnosis, ProblemCode::StaleSqliteRolloutPath) {
        for rollout in &report.rollout_records {
            if let Some(sqlite_row) = report
                .sqlite_threads
                .iter()
                .find(|thread| thread.id == rollout.thread_id)
            {
                let sqlite_points_to_missing_rollout = sqlite_row.rollout_path
                    != rollout.rollout_path
                    && !report
                        .rollout_records
                        .iter()
                        .any(|record| record.rollout_path == sqlite_row.rollout_path);

                if sqlite_points_to_missing_rollout {
                    plan.push_unique(RepairAction::RebuildMissingIndexFromRollout {
                        thread_id: rollout.thread_id.clone(),
                        rollout_path: rollout.rollout_path.clone(),
                    });
                }
            }
        }
    }

    if has_problem(diagnosis, ProblemCode::MissingSqliteThreadRow) {
        for rollout in &report.rollout_records {
            if report
                .sqlite_threads
                .iter()
                .all(|thread| thread.id != rollout.thread_id)
            {
                plan.push_unique(RepairAction::UpsertSqliteThreadMetadata {
                    thread_id: rollout.thread_id.clone(),
                });
            }
        }
    }

    if has_problem(diagnosis, ProblemCode::ArchivedStateMismatch) {
        for rollout in &report.rollout_records {
            if let Some(sqlite_row) = report
                .sqlite_threads
                .iter()
                .find(|thread| thread.id == rollout.thread_id)
            {
                let sqlite_archived = sqlite_row.archived_at.is_some();
                let rollout_archived =
                    matches!(rollout.location, ThreadLocation::Archived) || rollout.archived;
                if sqlite_archived != rollout_archived {
                    if sqlite_archived {
                        plan.push_unique(RepairAction::MoveRolloutToArchive {
                            thread_id: rollout.thread_id.clone(),
                        });
                    } else {
                        plan.push_unique(RepairAction::MoveRolloutToSessions {
                            thread_id: rollout.thread_id.clone(),
                        });
                    }
                }
            }
        }
    }

    if has_problem(diagnosis, ProblemCode::RolloutProviderMismatch) {
        for rollout in &report.rollout_records {
            if let Some(sqlite_row) = report
                .sqlite_threads
                .iter()
                .find(|thread| thread.id == rollout.thread_id)
            {
                if rollout.session_meta.provider.as_deref()
                    != Some(sqlite_row.model_provider.as_str())
                {
                    plan.push_unique(RepairAction::RewriteRolloutSessionMeta {
                        thread_id: rollout.thread_id.clone(),
                        provider: sqlite_row.model_provider.clone(),
                    });
                    plan.push_unique(RepairAction::UpsertSqliteThreadMetadata {
                        thread_id: rollout.thread_id.clone(),
                    });
                }
            }
        }
    }

    if has_problem(diagnosis, ProblemCode::MissingRootModelProvider) {
        if let Some(provider) = preferred_root_provider(report) {
            plan.push_unique(RepairAction::PatchConfigModelProvider { provider });
        }
    }

    plan
}

fn has_problem(diagnosis: &DiagnosisReport, code: ProblemCode) -> bool {
    diagnosis
        .problems
        .iter()
        .any(|problem| problem.code == code)
}

fn preferred_root_provider(report: &ScanReport) -> Option<String> {
    report
        .providers
        .rollout
        .keys()
        .next()
        .cloned()
        .or_else(|| report.providers.sqlite.keys().next().cloned())
        .or_else(|| {
            report
                .rollout_records
                .iter()
                .find_map(|record| record.session_meta.provider.clone())
        })
        .or_else(|| {
            report
                .sqlite_threads
                .iter()
                .map(|thread| thread.model_provider.clone())
                .next()
        })
}
