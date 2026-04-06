use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use doctor_core::{
    build_repair_plan, diagnose, execute_repair_plan, list_backups, list_repair_history,
    prune_backups, restore_backup, save_repair_history, scan_codex_home, BackupManifest,
    DiagnosisProblem, RepairActionRecord, RepairExecutionReport, RepairHistoryEntry, ScanReport,
};
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBannerKind {
    Hidden,
    Info,
    Success,
    Error,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Dashboard,
    Backups,
    History,
}

#[derive(Debug, Clone, Default)]
pub struct CodexDoctorApp {
    pub codex_home_input: String,
    pub dashboard: Option<DashboardViewModel>,
    pub last_error: Option<String>,
    pub preview_summary: String,
    pub status_message: String,
    pub last_operation_title: Option<String>,
    pub last_operation_at: Option<i64>,
    pub last_execution: Vec<RepairActionRecord>,
    pub backup_keep_latest_input: String,
    pub active_tab: Option<ActiveTab>,
    pub backups: Vec<BackupManifest>,
    pub history: Vec<RepairHistoryEntry>,
    pub last_backups_refresh_at: Option<i64>,
    pub last_history_refresh_at: Option<i64>,
    pub selected_backup: Option<usize>,
    pub selected_history: Option<usize>,
}

impl CodexDoctorApp {
    pub fn new(codex_home: String) -> Self {
        let mut app = Self {
            codex_home_input: codex_home,
            backup_keep_latest_input: "5".to_string(),
            active_tab: Some(ActiveTab::Dashboard),
            ..Self::default()
        };

        if !app.codex_home_input.trim().is_empty() {
            if let Err(error) = app.refresh() {
                app.last_error = Some(error);
            }
        }

        app
    }

    pub fn set_codex_home_input(&mut self, codex_home: String) {
        self.codex_home_input = codex_home;
    }

    fn codex_home_path(&self) -> Result<PathBuf, String> {
        let codex_home = PathBuf::from(&self.codex_home_input);
        if !codex_home.exists() {
            return Err(format!(
                "Codex home does not exist: {}",
                codex_home.display()
            ));
        }
        Ok(codex_home)
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        let codex_home = self.codex_home_path()?;
        let dashboard = load_dashboard_view_model(&codex_home)?;
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
        let codex_home = self.codex_home_path()?;
        let scan_report = scan_codex_home(&codex_home)?;
        let diagnosis = diagnose(&scan_report);
        let plan = build_repair_plan(&scan_report, &diagnosis);
        let backups_root = codex_home.join(".codex-doctor-backups");
        let execution = execute_repair_plan(&codex_home, &backups_root, &plan, false)?;
        let history_dir = codex_home.join(".codex-doctor").join("history");
        save_repair_history(&history_dir, &codex_home, &execution, &plan.actions)?;

        self.status_message = execution_status(&execution);
        self.last_operation_title = Some("Last repair".to_string());
        self.last_operation_at = Some(current_unix_timestamp_sec());
        self.last_execution = collect_execution_actions(&execution);
        self.last_error = None;
        self.refresh()?;
        self.load_backups()?;
        self.load_history()?;
        Ok(())
    }

    pub fn load_backups(&mut self) -> Result<(), String> {
        let codex_home = self.codex_home_path()?;
        let backups_root = codex_home.join(".codex-doctor-backups");
        self.backups = list_backups(&backups_root)?;
        self.selected_backup = None;
        Ok(())
    }

    pub fn refresh_backups(&mut self) -> Result<(), String> {
        self.load_backups()?;
        self.last_backups_refresh_at = Some(current_unix_timestamp_sec());
        self.status_message = format!("Loaded {} backup(s)", self.backups.len());
        self.last_error = None;
        Ok(())
    }

    pub fn load_history(&mut self) -> Result<(), String> {
        let codex_home = self.codex_home_path()?;
        let history_dir = codex_home.join(".codex-doctor").join("history");
        self.history = list_repair_history(&history_dir)?;
        self.selected_history = None;
        Ok(())
    }

    pub fn refresh_history(&mut self) -> Result<(), String> {
        self.load_history()?;
        self.last_history_refresh_at = Some(current_unix_timestamp_sec());
        self.status_message = format!("Loaded {} repair record(s)", self.history.len());
        self.last_error = None;
        Ok(())
    }

    pub fn restore_selected_backup(&mut self) -> Result<(), String> {
        if let Some(idx) = self.selected_backup {
            if let Some(manifest) = self.backups.get(idx) {
                let codex_home = self.codex_home_path()?;
                let backups_root = codex_home.join(".codex-doctor-backups");
                let snapshot_dir = backups_root.join(&manifest.backup_id);
                restore_backup(&snapshot_dir, &codex_home)?;
                self.status_message = format!("Restored backup: {}", manifest.backup_id);
                self.last_operation_title = Some("Last restore".to_string());
                self.last_operation_at = Some(current_unix_timestamp_sec());
                self.last_execution.clear();
                self.refresh()?;
                self.load_backups()?;
                self.load_history()?;
                return Ok(());
            }
        }
        Err("No backup selected".to_string())
    }

    pub fn prune_backups(&mut self, keep_latest: usize) -> Result<(), String> {
        let codex_home = self.codex_home_path()?;
        let backups_root = codex_home.join(".codex-doctor-backups");
        let report = prune_backups(&backups_root, keep_latest)?;
        self.load_backups()?;
        self.last_operation_title = Some("Last prune".to_string());
        self.last_operation_at = Some(current_unix_timestamp_sec());
        self.last_execution.clear();
        self.status_message = format!("Pruned {} backup(s)", report.removed_backup_ids.len());
        self.last_error = None;
        Ok(())
    }

    pub fn prune_backups_from_input(&mut self) -> Result<(), String> {
        let keep_latest = self
            .backup_keep_latest_input
            .trim()
            .parse::<usize>()
            .map_err(|_| "Keep latest must be a non-negative integer".to_string())?;
        self.prune_backups(keep_latest)
    }

    pub fn preview_actions(&self) -> &[String] {
        self.dashboard
            .as_ref()
            .map(|dashboard| dashboard.preview_actions.as_slice())
            .unwrap_or(&[])
    }

    pub fn error_clipboard_text(&self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn dashboard_clipboard_text(&self) -> Option<String> {
        let dashboard = self.dashboard.as_ref()?;
        let mut text = render_dashboard_text(dashboard);

        if let Some(last_operation) = self.last_operation_clipboard_text() {
            text.push_str("\nLast operation:\n");
            text.push_str(&last_operation);
        }

        Some(text)
    }

    pub fn last_operation_clipboard_text(&self) -> Option<String> {
        let title = self.last_operation_title.as_deref()?;
        let mut text = String::from(title);

        if let Some(timestamp) = self.last_operation_at {
            text.push_str("\nAt: ");
            text.push_str(&format_timestamp_sec(timestamp));
        }

        if self.last_execution.is_empty() {
            text.push_str("\nNo action-level details recorded for this operation.");
        } else {
            for action in &self.last_execution {
                text.push_str("\n- ");
                text.push_str(&action.action_type);
                text.push_str(": ");
                text.push_str(&action.details);
            }
        }

        Some(text)
    }

    pub fn export_dashboard_report(&mut self) -> Result<PathBuf, String> {
        let text = self
            .dashboard_clipboard_text()
            .ok_or_else(|| "No dashboard loaded".to_string())?;

        let exports_dir = self.prepare_exports_dir()?;
        let output_path = exports_dir.join("dashboard-report.txt");
        fs::write(&output_path, text).map_err(|err| err.to_string())?;
        self.status_message = format!("Exported report: {}", output_path.display());
        self.last_error = None;
        Ok(output_path)
    }

    pub fn prepare_exports_dir(&mut self) -> Result<PathBuf, String> {
        let codex_home = self.codex_home_path()?;
        let exports_dir = codex_home.join(".codex-doctor").join("exports");
        fs::create_dir_all(&exports_dir).map_err(|err| err.to_string())?;
        Ok(exports_dir)
    }

    pub fn open_exports_dir_with<F>(&mut self, opener: F) -> Result<PathBuf, String>
    where
        F: FnOnce(&Path) -> Result<(), String>,
    {
        let exports_dir = self.prepare_exports_dir()?;
        opener(&exports_dir)?;
        self.status_message = format!("Opened export folder: {}", exports_dir.display());
        self.last_error = None;
        Ok(exports_dir)
    }

    pub fn open_exports_dir(&mut self) -> Result<PathBuf, String> {
        self.open_exports_dir_with(open_path_in_file_manager)
    }

    pub fn export_last_operation_report(&mut self) -> Result<PathBuf, String> {
        let text = self
            .last_operation_clipboard_text()
            .ok_or_else(|| "No last operation available".to_string())?;

        let exports_dir = self.prepare_exports_dir()?;
        let output_path = exports_dir.join("last-operation-report.txt");
        fs::write(&output_path, text).map_err(|err| err.to_string())?;
        self.status_message = format!("Exported operation report: {}", output_path.display());
        self.last_error = None;
        Ok(output_path)
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
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🩺 Codex Doctor");
                ui.separator();
                ui.label("Diagnose and repair local Codex state");
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                match status_banner_kind(&self.status_message, self.last_error.as_deref()) {
                    StatusBannerKind::Hidden => {}
                    StatusBannerKind::Info => {
                        ui.colored_label(
                            egui::Color32::LIGHT_BLUE,
                            format!("ℹ️ {}", self.status_message),
                        );
                    }
                    StatusBannerKind::Success => {
                        ui.colored_label(
                            egui::Color32::GREEN,
                            format!("✅ {}", self.status_message),
                        );
                    }
                    StatusBannerKind::Error => {
                        if let Some(error) = &self.last_error {
                            ui.colored_label(egui::Color32::RED, format!("❌ {}", error));
                            if ui.button("📋 Copy error").clicked() {
                                ctx.copy_text(error.clone());
                            }
                        }
                    }
                }
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Settings");
            ui.separator();

            ui.label("Codex home:");
            ui.text_edit_singleline(&mut self.codex_home_input);

            if ui.button("🔄 Refresh").clicked() {
                if let Err(error) = self.refresh() {
                    self.last_error = Some(error);
                    self.dashboard = None;
                    self.preview_summary.clear();
                }
            }

            ui.separator();
            ui.heading("Navigation");

            if ui
                .selectable_label(
                    self.active_tab == Some(ActiveTab::Dashboard),
                    "📊 Dashboard",
                )
                .clicked()
            {
                self.active_tab = Some(ActiveTab::Dashboard);
            }

            if ui
                .selectable_label(self.active_tab == Some(ActiveTab::Backups), "💾 Backups")
                .clicked()
            {
                self.active_tab = Some(ActiveTab::Backups);
                if let Err(error) = self.load_backups() {
                    self.last_error = Some(error);
                }
            }

            if ui
                .selectable_label(self.active_tab == Some(ActiveTab::History), "📜 History")
                .clicked()
            {
                self.active_tab = Some(ActiveTab::History);
                if let Err(error) = self.load_history() {
                    self.last_error = Some(error);
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab.as_ref().unwrap_or(&ActiveTab::Dashboard) {
                ActiveTab::Dashboard => self.render_dashboard_tab(ui, ctx),
                ActiveTab::Backups => self.render_backups_tab(ui),
                ActiveTab::History => self.render_history_tab(ui),
            }
        });
    }
}

impl CodexDoctorApp {
    fn render_dashboard_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Summary");

            if let Some(dashboard) = self.dashboard.clone() {
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
                    ui.colored_label(egui::Color32::GREEN, "✅ No problems detected.");
                } else {
                    for problem in &dashboard.problems {
                        ui.group(|ui| {
                            let color = match problem.severity.as_str() {
                                "error" => egui::Color32::RED,
                                "warning" => egui::Color32::YELLOW,
                                _ => egui::Color32::LIGHT_BLUE,
                            };
                            ui.colored_label(
                                color,
                                format!("{} ({})", problem.code, problem.severity),
                            );
                            for evidence in &problem.evidence {
                                ui.label(format!("  • {}", evidence));
                            }
                        });
                    }
                }

                ui.separator();
                ui.heading("Repair plan");

                if !self.preview_summary.is_empty() {
                    ui.label(&self.preview_summary);
                }

                if ui.button("📋 Copy summary").clicked() {
                    if let Some(text) = self.dashboard_clipboard_text() {
                        ctx.copy_text(text);
                    }
                }
                if ui.button("💾 Export report").clicked() {
                    if let Err(error) = self.export_dashboard_report() {
                        self.last_error = Some(error);
                    }
                }
                if ui.button("📂 Open export folder").clicked() {
                    if let Err(error) = self.open_exports_dir() {
                        self.last_error = Some(error);
                    }
                }

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

                if self.last_operation_title.is_some() {
                    ui.separator();
                    ui.heading(
                        self.last_operation_title
                            .as_deref()
                            .unwrap_or("Last execution"),
                    );
                    if let Some(timestamp) = self.last_operation_at {
                        ui.label(format!("At: {}", format_timestamp_sec(timestamp)));
                    }
                    if ui.button("💾 Export last operation").clicked() {
                        if let Err(error) = self.export_last_operation_report() {
                            self.last_error = Some(error);
                        }
                    }
                    if ui.button("📋 Copy last operation").clicked() {
                        if let Some(text) = self.last_operation_clipboard_text() {
                            ctx.copy_text(text);
                        }
                    }
                    if self.last_execution.is_empty() {
                        ui.label("No action-level details recorded for this operation.");
                    } else {
                        for action in &self.last_execution {
                            render_action_record(ui, action);
                        }
                    }
                }
            } else {
                ui.label("Press Refresh to load the current Codex home.");
            }
        });
    }

    fn render_backups_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Backups");
                if ui.button("🔄 Refresh").clicked() {
                    if let Err(error) = self.refresh_backups() {
                        self.last_error = Some(error);
                    }
                }
            });
            if let Some(timestamp) = self.last_backups_refresh_at {
                ui.label(format!(
                    "Last refreshed: {}",
                    format_timestamp_sec(timestamp)
                ));
            }

            ui.horizontal(|ui| {
                ui.label("Keep latest:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.backup_keep_latest_input)
                        .desired_width(64.0),
                );
                if ui.button("🗑️ Prune").clicked() {
                    if let Err(error) = self.prune_backups_from_input() {
                        self.last_error = Some(error);
                    }
                }
            });
            ui.separator();

            if self.backups.is_empty() {
                ui.label("No backups found.");
            } else {
                ui.label(format!("Found {} backup(s)", self.backups.len()));
                ui.separator();

                for (i, manifest) in self.backups.iter().enumerate() {
                    let is_selected = self.selected_backup == Some(i);
                    if ui
                        .selectable_label(is_selected, format!("Backup: {}", manifest.backup_id))
                        .clicked()
                    {
                        self.selected_backup = Some(i);
                    }

                    if is_selected {
                        ui.indent(format!("backup_details_{}", i), |ui| {
                            ui.label(format!("Source: {}", manifest.source_codex_home.display()));
                            ui.label(format!(
                                "Created: {}",
                                format_timestamp_ms(manifest.created_at_unix_ms)
                            ));
                        });
                    }
                }

                ui.separator();
                if ui
                    .add_enabled(
                        self.selected_backup.is_some(),
                        egui::Button::new("🔄 Restore Selected"),
                    )
                    .clicked()
                {
                    if let Err(error) = self.restore_selected_backup() {
                        self.last_error = Some(error);
                    }
                }
            }
        });
    }

    fn render_history_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Repair History");
                if ui.button("🔄 Refresh").clicked() {
                    if let Err(error) = self.refresh_history() {
                        self.last_error = Some(error);
                    }
                }
            });
            if let Some(timestamp) = self.last_history_refresh_at {
                ui.label(format!(
                    "Last refreshed: {}",
                    format_timestamp_sec(timestamp)
                ));
            }

            if self.history.is_empty() {
                ui.label("No repair history found.");
            } else {
                ui.label(format!("Found {} repair(s)", self.history.len()));
                ui.separator();

                for (i, entry) in self.history.iter().enumerate() {
                    let is_selected = self.selected_history == Some(i);
                    if ui
                        .selectable_label(
                            is_selected,
                            format!("Repair: {}", format_timestamp_sec(entry.timestamp)),
                        )
                        .clicked()
                    {
                        self.selected_history = Some(i);
                    }

                    if is_selected {
                        ui.indent(format!("history_details_{}", i), |ui| {
                            ui.label(format!("Codex home: {}", entry.codex_home.display()));
                            ui.label(format!(
                                "Actions: {} applied, {} skipped, {} failed",
                                entry.actions_applied, entry.actions_skipped, entry.actions_failed
                            ));
                            if let Some(backup_id) = &entry.backup_id {
                                ui.label(format!("Backup: {}", backup_id));
                            }

                            if !entry.actions.is_empty() {
                                ui.separator();
                                ui.label("Actions:");
                                for action in &entry.actions {
                                    render_action_record(ui, action);
                                }
                            }
                        });
                    }
                }
            }
        });
    }
}

fn render_action_record(ui: &mut egui::Ui, action: &RepairActionRecord) {
    let (icon, color) = match action.status {
        doctor_core::ActionStatus::Applied => ("✓", egui::Color32::GREEN),
        doctor_core::ActionStatus::Skipped => ("○", egui::Color32::GRAY),
        doctor_core::ActionStatus::Failed => ("✗", egui::Color32::RED),
    };
    ui.horizontal(|ui| {
        ui.colored_label(color, icon);
        ui.label(format!("{} - {}", action.action_type, action.details));
    });
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

fn collect_execution_actions(report: &RepairExecutionReport) -> Vec<RepairActionRecord> {
    let mut actions = Vec::new();

    for entry in &report.applied {
        actions.push(RepairActionRecord {
            action_type: action_id(&entry.action),
            thread_id: action_thread_id(&entry.action),
            details: entry.message.clone(),
            status: doctor_core::ActionStatus::Applied,
        });
    }
    for entry in &report.skipped {
        actions.push(RepairActionRecord {
            action_type: action_id(&entry.action),
            thread_id: action_thread_id(&entry.action),
            details: entry.message.clone(),
            status: doctor_core::ActionStatus::Skipped,
        });
    }
    for entry in &report.failed {
        actions.push(RepairActionRecord {
            action_type: action_id(&entry.action),
            thread_id: action_thread_id(&entry.action),
            details: entry.message.clone(),
            status: doctor_core::ActionStatus::Failed,
        });
    }

    actions
}

fn current_unix_timestamp_sec() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}

fn open_path_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn status_banner_kind(status_message: &str, last_error: Option<&str>) -> StatusBannerKind {
    if last_error.is_some() {
        return StatusBannerKind::Error;
    }
    if status_message.is_empty() {
        return StatusBannerKind::Hidden;
    }
    if status_message.starts_with("Applied:")
        || status_message.starts_with("Restored")
        || status_message.starts_with("Pruned")
    {
        return StatusBannerKind::Success;
    }
    StatusBannerKind::Info
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

fn action_thread_id(action: &doctor_core::RepairAction) -> Option<String> {
    match action {
        doctor_core::RepairAction::RebuildMissingIndexFromRollout { thread_id, .. }
        | doctor_core::RepairAction::UpsertSqliteThreadMetadata { thread_id }
        | doctor_core::RepairAction::MoveRolloutToArchive { thread_id }
        | doctor_core::RepairAction::MoveRolloutToSessions { thread_id }
        | doctor_core::RepairAction::RewriteRolloutSessionMeta { thread_id, .. } => {
            Some(thread_id.clone())
        }
        doctor_core::RepairAction::PatchConfigModelProvider { .. } => None,
    }
}

fn format_timestamp_ms(millis: u128) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let duration = Duration::from_millis(millis as u64);
    let datetime = UNIX_EPOCH + duration;
    let datetime: chrono::DateTime<chrono::Local> = datetime.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn format_timestamp_sec(secs: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let duration = Duration::from_secs(secs as u64);
    let datetime = UNIX_EPOCH + duration;
    let datetime: chrono::DateTime<chrono::Local> = datetime.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}
