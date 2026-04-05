use std::path::{Path, PathBuf};

use doctor_core::{
    build_repair_plan, diagnose, execute_repair_plan, scan_codex_home, DiagnosisProblem,
    RepairExecutionReport, ScanReport,
};
use eframe::egui;

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
        preview_actions: repair_plan.actions.iter().map(action_id).collect(),
    })
}

#[derive(Debug, Clone, Default)]
pub struct CodexDoctorApp {
    pub codex_home_input: String,
    pub dashboard: Option<DashboardViewModel>,
    pub last_error: Option<String>,
    pub preview_summary: String,
    pub status_message: String,
}

impl CodexDoctorApp {
    pub fn new(codex_home: String) -> Self {
        Self {
            codex_home_input: codex_home,
            ..Self::default()
        }
    }

    pub fn set_codex_home_input(&mut self, codex_home: String) {
        self.codex_home_input = codex_home;
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        let dashboard = load_dashboard_view_model(Path::new(&self.codex_home_input))?;
        self.preview_summary.clear();
        self.last_error = None;
        self.dashboard = Some(dashboard);
        Ok(())
    }

    pub fn preview_repair(&mut self) -> Result<(), String> {
        if self.dashboard.is_none() {
            self.refresh()?;
        }

        let actions = self.preview_actions().to_vec();
        self.preview_summary = render_preview_summary(&actions);
        self.status_message = format!("Previewed: {}", actions.len());
        self.last_error = None;
        Ok(())
    }

    pub fn execute_repair(&mut self) -> Result<(), String> {
        let codex_home = PathBuf::from(&self.codex_home_input);
        let scan_report = scan_codex_home(&codex_home)?;
        let diagnosis = diagnose(&scan_report);
        let plan = build_repair_plan(&scan_report, &diagnosis);
        let backups_root = codex_home.join(".codex-doctor-backups");
        let execution = execute_repair_plan(&codex_home, &backups_root, &plan, false)?;

        self.status_message = execution_status(&execution);
        self.last_error = None;
        self.refresh()?;
        Ok(())
    }

    pub fn preview_actions(&self) -> &[String] {
        self.dashboard
            .as_ref()
            .map(|dashboard| dashboard.preview_actions.as_slice())
            .unwrap_or(&[])
    }

    pub fn execute_repair_label(&self) -> &'static str {
        "Execute repair"
    }

    pub fn preview_repair_label(&self) -> &'static str {
        "Preview repair"
    }
}

impl eframe::App for CodexDoctorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Codex Doctor");
            ui.label("Diagnose and repair local Codex state.");

            ui.horizontal(|ui| {
                ui.label("Codex home");
                ui.text_edit_singleline(&mut self.codex_home_input);
                if ui.button("Refresh").clicked() {
                    if let Err(error) = self.refresh() {
                        self.last_error = Some(error);
                        self.dashboard = None;
                        self.preview_summary.clear();
                    }
                }
            });

            if !self.status_message.is_empty() {
                ui.label(&self.status_message);
            }

            if let Some(error) = &self.last_error {
                ui.colored_label(egui::Color32::RED, error);
            }

            ui.separator();
            ui.heading("Summary");
            if let Some(dashboard) = &self.dashboard {
                egui::Grid::new("summary-grid")
                    .striped(true)
                    .show(ui, |ui| {
                        for item in &dashboard.summary_items {
                            ui.label(&item.label);
                            ui.label(&item.value);
                            ui.end_row();
                        }
                    });

                ui.separator();
                ui.heading("Problems");
                if dashboard.problems.is_empty() {
                    ui.label("No problems detected.");
                } else {
                    for problem in &dashboard.problems {
                        ui.group(|ui| {
                            ui.label(format!("{} ({})", problem.code, problem.severity));
                            for evidence in &problem.evidence {
                                ui.label(evidence);
                            }
                        });
                    }
                }

                ui.separator();
                ui.heading("Repair plan");
                ui.label(&self.preview_summary);
                let has_actions = !dashboard.preview_actions.is_empty();
                ui.horizontal(|ui| {
                    if ui.button(self.preview_repair_label()).clicked() {
                        if let Err(error) = self.preview_repair() {
                            self.last_error = Some(error);
                        }
                    }
                    if ui
                        .add_enabled(has_actions, egui::Button::new(self.execute_repair_label()))
                        .clicked()
                    {
                        if let Err(error) = self.execute_repair() {
                            self.last_error = Some(error);
                        }
                    }
                });
            } else {
                ui.label("Press Refresh to load the current Codex home.");
            }
        });
    }
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

fn execution_status(report: &RepairExecutionReport) -> String {
    format!(
        "Applied: {}, Skipped: {}, Failed: {}",
        report.applied.len(),
        report.skipped.len(),
        report.failed.len()
    )
}

fn render_preview_summary(preview_actions: &[String]) -> String {
    if preview_actions.is_empty() {
        "No repair actions required.".to_string()
    } else {
        format!("Preview actions: {}", preview_actions.join(", "))
    }
}

fn build_summary_items(
    scan_report: &ScanReport,
    problem_count: usize,
) -> Vec<SummaryItemViewModel> {
    let preview_problem_count = if problem_count == 0 && scan_report.summary.root_provider.is_none()
    {
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
