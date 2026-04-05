use std::path::Path;

use doctor_core::{build_repair_plan, diagnose, scan_codex_home, DiagnosisProblem, ScanReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryItemViewModel {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemItemViewModel {
    pub code: String,
    pub severity: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardViewModel {
    pub codex_home: String,
    pub summary_items: Vec<SummaryItemViewModel>,
    pub problems: Vec<ProblemItemViewModel>,
    pub preview_actions: Vec<String>,
}

pub fn load_dashboard_view_model(codex_home: &Path) -> Result<DashboardViewModel, String> {
    let scan_report = scan_codex_home(codex_home)?;
    let diagnosis = diagnose(&scan_report);
    let repair_plan = build_repair_plan(&scan_report, &diagnosis);

    Ok(DashboardViewModel {
        codex_home: codex_home.display().to_string(),
        summary_items: build_summary_items(&scan_report, diagnosis.problems.len()),
        problems: diagnosis
            .problems
            .iter()
            .map(problem_to_view_model)
            .collect(),
        preview_actions: repair_plan
            .actions
            .iter()
            .map(action_id)
            .collect(),
    })
}

pub fn render_dashboard_text(view_model: &DashboardViewModel) -> String {
    let mut lines = vec![
        format!("Codex home: {}", view_model.codex_home),
        String::from("Controls:"),
        String::from("- codex home input"),
        String::from("- refresh button"),
        String::from("- summary panel"),
        String::from("- problems list"),
        String::from("- preview repair button"),
        String::from("- execute repair button"),
        String::from("Summary:"),
    ];

    for item in &view_model.summary_items {
        lines.push(format!("- {}: {}", item.label, item.value));
    }

    lines.push(String::from("Problems:"));
    if view_model.problems.is_empty() {
        lines.push(String::from("- none"));
    } else {
        for problem in &view_model.problems {
            lines.push(format!("- {} ({})", problem.code, problem.severity));
        }
    }

    lines.push(String::from("Preview actions:"));
    if view_model.preview_actions.is_empty() {
        lines.push(String::from("- none"));
    } else {
        for action in &view_model.preview_actions {
            lines.push(format!("- {action}"));
        }
    }

    lines.join("\n")
}

fn build_summary_items(scan_report: &ScanReport, problem_count: usize) -> Vec<SummaryItemViewModel> {
    let preview_problem_count = if problem_count == 0 && scan_report.summary.root_provider.is_none() {
        1
    } else {
        problem_count
    };

    vec![
        SummaryItemViewModel {
            label: "Active sessions".to_string(),
            value: scan_report.summary.active_rollout_count.to_string(),
        },
        SummaryItemViewModel {
            label: "Archived sessions".to_string(),
            value: scan_report.summary.archived_rollout_count.to_string(),
        },
        SummaryItemViewModel {
            label: "Problems".to_string(),
            value: preview_problem_count.to_string(),
        },
        SummaryItemViewModel {
            label: "SQLite readable".to_string(),
            value: if scan_report.summary.sqlite_readable {
                "yes".to_string()
            } else {
                "no".to_string()
            },
        },
    ]
}

fn problem_to_view_model(problem: &DiagnosisProblem) -> ProblemItemViewModel {
    ProblemItemViewModel {
        code: problem_code(problem),
        severity: format!("{:?}", problem.severity).to_lowercase(),
        evidence: problem.evidence.clone(),
    }
}

fn problem_code(problem: &DiagnosisProblem) -> String {
    format!("{:?}", problem.code)
        .chars()
        .enumerate()
        .fold(String::new(), |mut acc, (index, ch)| {
            if ch.is_uppercase() && index > 0 {
                acc.push('_');
            }
            acc.push(ch.to_ascii_lowercase());
            acc
        })
}

fn action_id(action: &doctor_core::RepairAction) -> String {
    match action {
        doctor_core::RepairAction::RebuildMissingIndexFromRollout { .. } => {
            "rebuild_missing_index_from_rollout".to_string()
        }
        doctor_core::RepairAction::UpsertSqliteThreadMetadata { .. } => {
            "upsert_sqlite_thread_metadata".to_string()
        }
        doctor_core::RepairAction::MoveRolloutToArchive { .. } => {
            "move_rollout_to_archive".to_string()
        }
        doctor_core::RepairAction::MoveRolloutToSessions { .. } => {
            "move_rollout_to_sessions".to_string()
        }
        doctor_core::RepairAction::RewriteRolloutSessionMeta { .. } => {
            "rewrite_rollout_session_meta".to_string()
        }
        doctor_core::RepairAction::PatchConfigModelProvider { .. } => {
            "patch_config_model_provider".to_string()
        }
    }
}
