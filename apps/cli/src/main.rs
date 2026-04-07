mod output;

use std::env;
use std::path::PathBuf;
use std::process;

use clap::{Args, Parser, Subcommand};
use doctor_core::{
    build_repair_plan, build_resume_doctor_report, diagnose, execute_repair_plan_with_sqlite_home,
    list_backups, list_repair_history, prune_backups, restore_backup_with_sqlite_home,
    save_repair_history, scan_codex_home_with_sqlite_home, BackupManifest, BackupSnapshot,
    DiagnosisProblem, DiagnosisReport, ProblemCode, ProblemSeverity, RepairAction,
    RepairExecutionEntry, RepairExecutionReport, RepairPlan, ResumeBlocker, ResumeCandidate,
    ResumeDoctorReport, RolloutRecord, ScanReport, SqliteThreadRecord, ThreadLocation,
};
use serde_json::{json, Value};

use output::{
    print_backup_list_human, print_diagnosis_report_human, print_repair_execution_human,
    print_repair_history_human, print_resume_doctor_human, print_scan_report_human,
};

#[derive(Parser)]
#[command(name = "codex-doctor")]
#[command(version, about = "Diagnose and repair local Codex state", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Scan(ScanArgs),
    Diagnose(DiagnoseArgs),
    ResumeDoctor(ResumeDoctorArgs),
    Repair(RepairArgs),
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },
    History(HistoryArgs),
}

#[derive(Args)]
struct ScanArgs {
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    sqlite_home: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct DiagnoseArgs {
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    sqlite_home: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ResumeDoctorArgs {
    #[arg(long)]
    codex_home: Option<PathBuf>,
    #[arg(long)]
    sqlite_home: Option<PathBuf>,
    #[arg(long)]
    current_cwd: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct RepairArgs {
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    sqlite_home: Option<PathBuf>,
    #[arg(long)]
    backups_root: PathBuf,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value = "false")]
    save_history: bool,
}

#[derive(Subcommand)]
enum BackupCommands {
    List(BackupListArgs),
    Restore(BackupRestoreArgs),
    Prune(BackupPruneArgs),
}

#[derive(Args)]
struct BackupListArgs {
    #[arg(long)]
    backups_root: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct BackupRestoreArgs {
    #[arg(long)]
    snapshot_dir: PathBuf,
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    sqlite_home: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct BackupPruneArgs {
    #[arg(long)]
    backups_root: PathBuf,
    #[arg(long)]
    keep_latest: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct HistoryArgs {
    #[arg(long)]
    history_dir: PathBuf,
    #[arg(long)]
    json: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan(args) => {
            let report =
                scan_codex_home_with_sqlite_home(&args.codex_home, args.sqlite_home.as_deref())?;
            if args.json {
                print_json(&scan_report_to_json(&report))?;
            } else {
                print_scan_report_human(&report);
            }
        }
        Commands::Diagnose(args) => {
            let report =
                scan_codex_home_with_sqlite_home(&args.codex_home, args.sqlite_home.as_deref())?;
            let diagnosis = diagnose(&report);
            if args.json {
                print_json(&diagnosis_to_json(&diagnosis))?;
            } else {
                print_diagnosis_report_human(&diagnosis.problems);
            }
        }
        Commands::ResumeDoctor(args) => {
            let codex_home = match args.codex_home {
                Some(path) => path,
                None => default_codex_home()?,
            };
            let report =
                scan_codex_home_with_sqlite_home(&codex_home, args.sqlite_home.as_deref())?;
            let current_cwd = match args.current_cwd {
                Some(path) => path,
                None => env::current_dir().map_err(|err| err.to_string())?,
            };
            let resume_report = build_resume_doctor_report(&report, &current_cwd);
            if args.json {
                print_json(&resume_doctor_report_to_json(&resume_report))?;
            } else {
                print_resume_doctor_human(&resume_report);
            }
        }
        Commands::Repair(args) => {
            let report =
                scan_codex_home_with_sqlite_home(&args.codex_home, args.sqlite_home.as_deref())?;
            let diagnosis = diagnose(&report);
            let plan = build_repair_plan(&report, &diagnosis);
            let execution_report = execute_repair_plan_with_sqlite_home(
                &args.codex_home,
                &args.backups_root,
                &plan,
                args.dry_run,
                args.sqlite_home.as_deref(),
            )?;

            if args.save_history && !args.dry_run {
                let history_dir = args.codex_home.join(".codex-doctor").join("history");
                save_repair_history(
                    &history_dir,
                    &args.codex_home,
                    &execution_report,
                    &plan.actions,
                )?;
            }

            if args.json {
                print_json(&repair_execution_report_to_json(&execution_report, &plan))?;
            } else {
                let retryable_hint = execution_report
                    .failed
                    .iter()
                    .chain(execution_report.skipped.iter())
                    .find_map(|entry| {
                        if entry.retryable {
                            retryable_hint(&entry.message)
                        } else {
                            None
                        }
                    });
                print_repair_execution_human(
                    execution_report.applied.len(),
                    execution_report.skipped.len(),
                    execution_report.failed.len(),
                    execution_report
                        .backup
                        .as_ref()
                        .map(|b| b.backup_id.as_str()),
                    retryable_hint,
                );
            }
        }
        Commands::Backup { command } => match command {
            BackupCommands::List(args) => {
                let manifests = list_backups(&args.backups_root)?;
                if args.json {
                    print_json(&Value::Array(
                        manifests.iter().map(backup_manifest_to_json).collect(),
                    ))?;
                } else {
                    print_backup_list_human(&manifests);
                }
            }
            BackupCommands::Restore(args) => {
                restore_backup_with_sqlite_home(
                    &args.snapshot_dir,
                    &args.codex_home,
                    args.sqlite_home.as_deref(),
                )?;
                if args.json {
                    print_json(&json!({
                        "snapshot_dir": args.snapshot_dir,
                        "codex_home": args.codex_home,
                        "sqlite_home": args.sqlite_home,
                        "restored": true,
                    }))?;
                } else {
                    println!("✅ Backup restored successfully!");
                }
            }
            BackupCommands::Prune(args) => {
                let report = prune_backups(&args.backups_root, args.keep_latest)?;
                if args.json {
                    print_json(&json!({
                        "removed_backup_ids": report.removed_backup_ids,
                    }))?;
                } else {
                    println!("🗑️  Pruned {} backup(s)", report.removed_backup_ids.len());
                }
            }
        },
        Commands::History(args) => {
            let entries = list_repair_history(&args.history_dir)?;
            if args.json {
                print_json(&Value::Array(
                    entries.iter().map(repair_history_entry_to_json).collect(),
                ))?;
            } else {
                print_repair_history_human(&entries);
            }
        }
    }

    Ok(())
}

fn default_codex_home() -> Result<PathBuf, String> {
    let home = env::var_os("CODEX_HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "could not resolve codex home; pass --codex-home explicitly".to_string())?;

    Ok(home.join(".codex"))
}

fn print_json(value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    println!("{text}");
    Ok(())
}

fn scan_report_to_json(report: &ScanReport) -> Value {
    json!({
        "summary": {
            "config_present": report.summary.config_present,
            "sessions_present": report.summary.sessions_present,
            "sqlite_present": report.summary.sqlite_present,
            "sqlite_readable": report.summary.sqlite_readable,
            "sqlite_locked": report.summary.sqlite_locked,
            "logs_present": report.summary.logs_present,
            "logs_readable": report.summary.logs_readable,
            "history_present": report.summary.history_present,
            "history_readable": report.summary.history_readable,
            "active_rollout_count": report.summary.active_rollout_count,
            "archived_rollout_count": report.summary.archived_rollout_count,
            "locked_rollout_count": report.summary.locked_rollout_count,
            "root_provider": report.summary.root_provider,
        },
        "providers": {
            "rollout": report.providers.rollout,
            "sqlite": report.providers.sqlite,
        },
        "root_config": report.root_config.as_ref().map(|config| json!({
            "model_provider": config.model_provider,
        })),
        "rollout_records": report
            .rollout_records
            .iter()
            .map(rollout_record_to_json)
            .collect::<Vec<_>>(),
        "sqlite_threads": report
            .sqlite_threads
            .iter()
            .map(sqlite_thread_to_json)
            .collect::<Vec<_>>(),
    })
}

fn diagnosis_to_json(diagnosis: &DiagnosisReport) -> Value {
    json!({
        "problems": diagnosis
            .problems
            .iter()
            .map(diagnosis_problem_to_json)
            .collect::<Vec<_>>(),
    })
}

fn resume_doctor_report_to_json(report: &ResumeDoctorReport) -> Value {
    json!({
        "current_cwd": report.current_cwd,
        "root_provider": report.root_provider,
        "candidates": report.candidates.iter().map(resume_candidate_to_json).collect::<Vec<_>>(),
    })
}

fn repair_execution_report_to_json(report: &RepairExecutionReport, plan: &RepairPlan) -> Value {
    json!({
        "backup": report.backup.as_ref().map(backup_snapshot_to_json),
        "plan": {
            "actions": plan.actions.iter().map(repair_action_to_json).collect::<Vec<_>>(),
            "dry_run_summary": plan.render_dry_run_summary(),
        },
        "applied": report.applied.iter().map(repair_execution_entry_to_json).collect::<Vec<_>>(),
        "skipped": report.skipped.iter().map(repair_execution_entry_to_json).collect::<Vec<_>>(),
        "failed": report.failed.iter().map(repair_execution_entry_to_json).collect::<Vec<_>>(),
    })
}

fn diagnosis_problem_to_json(problem: &DiagnosisProblem) -> Value {
    json!({
        "code": problem_code_to_str(&problem.code),
        "severity": problem_severity_to_str(&problem.severity),
        "evidence": problem.evidence,
        "suggested_fix_ids": problem.suggested_fix_ids,
    })
}

fn resume_candidate_to_json(candidate: &ResumeCandidate) -> Value {
    json!({
        "thread_id": candidate.thread_id,
        "provider": candidate.provider,
        "cwd": candidate.cwd,
        "location": thread_location_to_str(&candidate.location),
        "default_picker_visible": candidate.default_picker_visible,
        "blockers": candidate.blockers.iter().map(resume_blocker_to_json).collect::<Vec<_>>(),
        "direct_resume_command": candidate.direct_resume_command,
    })
}

fn resume_blocker_to_json(blocker: &ResumeBlocker) -> Value {
    match blocker {
        ResumeBlocker::MissingSqliteThreadRow => json!({
            "type": "missing_sqlite_thread_row"
        }),
        ResumeBlocker::Archived => json!({
            "type": "archived"
        }),
        ResumeBlocker::ProviderMismatch {
            session_provider,
            current_provider,
        } => json!({
            "type": "provider_mismatch",
            "session_provider": session_provider,
            "current_provider": current_provider,
        }),
        ResumeBlocker::CwdMismatch {
            session_cwd,
            current_cwd,
        } => json!({
            "type": "cwd_mismatch",
            "session_cwd": session_cwd,
            "current_cwd": current_cwd,
        }),
    }
}

fn repair_execution_entry_to_json(entry: &RepairExecutionEntry) -> Value {
    json!({
        "action": repair_action_to_json(&entry.action),
        "message": entry.message,
        "retryable": entry.retryable,
    })
}

fn backup_snapshot_to_json(snapshot: &BackupSnapshot) -> Value {
    json!({
        "backup_id": snapshot.backup_id,
        "snapshot_dir": snapshot.snapshot_dir,
        "manifest": backup_manifest_to_json(&snapshot.manifest),
    })
}

fn backup_manifest_to_json(manifest: &BackupManifest) -> Value {
    json!({
        "backup_id": manifest.backup_id,
        "source_codex_home": manifest.source_codex_home,
        "created_at_unix_ms": manifest.created_at_unix_ms,
    })
}

fn rollout_record_to_json(record: &RolloutRecord) -> Value {
    json!({
        "thread_id": record.thread_id,
        "rollout_path": record.rollout_path,
        "session_meta": {
            "provider": record.session_meta.provider,
            "cwd": record.session_meta.cwd,
            "timestamp": record.session_meta.timestamp,
        },
        "location": thread_location_to_str(&record.location),
        "archived": record.archived,
    })
}

fn sqlite_thread_to_json(thread: &SqliteThreadRecord) -> Value {
    json!({
        "id": thread.id,
        "rollout_path": thread.rollout_path,
        "model_provider": thread.model_provider,
        "archived_at": thread.archived_at,
        "cwd": thread.cwd,
    })
}

fn repair_action_to_json(action: &RepairAction) -> Value {
    match action {
        RepairAction::RebuildMissingIndexFromRollout {
            thread_id,
            rollout_path,
        } => json!({
            "type": "rebuild_missing_index_from_rollout",
            "thread_id": thread_id,
            "rollout_path": rollout_path,
        }),
        RepairAction::UpsertSqliteThreadMetadata { thread_id } => json!({
            "type": "upsert_sqlite_thread_metadata",
            "thread_id": thread_id,
        }),
        RepairAction::MoveRolloutToArchive { thread_id } => json!({
            "type": "move_rollout_to_archive",
            "thread_id": thread_id,
        }),
        RepairAction::MoveRolloutToSessions { thread_id } => json!({
            "type": "move_rollout_to_sessions",
            "thread_id": thread_id,
        }),
        RepairAction::RewriteRolloutSessionMeta {
            thread_id,
            provider,
        } => json!({
            "type": "rewrite_rollout_session_meta",
            "thread_id": thread_id,
            "provider": provider,
        }),
        RepairAction::PatchConfigModelProvider { provider } => json!({
            "type": "patch_config_model_provider",
            "provider": provider,
        }),
    }
}

fn repair_history_entry_to_json(entry: &doctor_core::RepairHistoryEntry) -> Value {
    json!({
        "timestamp": entry.timestamp,
        "codex_home": entry.codex_home,
        "actions_applied": entry.actions_applied,
        "actions_skipped": entry.actions_skipped,
        "actions_failed": entry.actions_failed,
        "backup_id": entry.backup_id,
        "actions": entry.actions.iter().map(|a| json!({
            "action_type": a.action_type,
            "thread_id": a.thread_id,
            "details": a.details,
            "retryable": a.retryable,
            "status": match a.status {
                doctor_core::ActionStatus::Applied => "applied",
                doctor_core::ActionStatus::Skipped => "skipped",
                doctor_core::ActionStatus::Failed => "failed",
            }
        })).collect::<Vec<_>>(),
    })
}

fn problem_code_to_str(code: &ProblemCode) -> &'static str {
    match code {
        ProblemCode::MissingSessionsDirectory => "missing_sessions_directory",
        ProblemCode::UnreadableSqliteDatabase => "unreadable_sqlite_database",
        ProblemCode::LockedDatabase => "locked_database",
        ProblemCode::LockedRolloutFile => "locked_rollout_file",
        ProblemCode::MissingSqliteThreadRow => "missing_sqlite_thread_row",
        ProblemCode::StaleSqliteRolloutPath => "stale_sqlite_rollout_path",
        ProblemCode::RolloutProviderMismatch => "rollout_provider_mismatch",
        ProblemCode::ArchivedStateMismatch => "archived_state_mismatch",
        ProblemCode::ResumePickerProviderFiltered => "resume_picker_provider_filtered",
        ProblemCode::ResumePickerArchivedFiltered => "resume_picker_archived_filtered",
        ProblemCode::MissingRootModelProvider => "missing_root_model_provider",
        ProblemCode::MissingLogsSqlite => "missing_logs_sqlite",
        ProblemCode::UnreadableLogsSqlite => "unreadable_logs_sqlite",
        ProblemCode::MissingHistoryJsonl => "missing_history_jsonl",
        ProblemCode::UnreadableHistoryJsonl => "unreadable_history_jsonl",
    }
}

fn retryable_hint(message: &str) -> Option<&'static str> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("locked")
        || lower.contains("busy")
        || lower.contains("used by another process")
    {
        Some("close Codex or any process holding the file/database, then retry the operation")
    } else {
        None
    }
}

fn problem_severity_to_str(severity: &ProblemSeverity) -> &'static str {
    match severity {
        ProblemSeverity::Info => "info",
        ProblemSeverity::Warning => "warning",
        ProblemSeverity::Error => "error",
    }
}

fn thread_location_to_str(location: &ThreadLocation) -> &'static str {
    match location {
        ThreadLocation::Active => "active",
        ThreadLocation::Archived => "archived",
    }
}
