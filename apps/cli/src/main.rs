use std::path::PathBuf;
use std::process;

use clap::{Args, Parser, Subcommand};
use doctor_core::{
    build_repair_plan, diagnose, execute_repair_plan, list_backups, prune_backups, restore_backup,
    scan_codex_home, BackupManifest, BackupSnapshot, DiagnosisProblem, DiagnosisReport,
    ProblemCode, ProblemSeverity, RepairAction, RepairExecutionEntry, RepairExecutionReport,
    RepairPlan, RolloutRecord, ScanReport, SqliteThreadRecord, ThreadLocation,
};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "codex-doctor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Scan(ScanArgs),
    Diagnose(DiagnoseArgs),
    Repair(RepairArgs),
    Backup {
        #[command(subcommand)]
        command: BackupCommands,
    },
}

#[derive(Args)]
struct ScanArgs {
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct DiagnoseArgs {
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct RepairArgs {
    #[arg(long)]
    codex_home: PathBuf,
    #[arg(long)]
    backups_root: PathBuf,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    json: bool,
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
            let report = scan_codex_home(&args.codex_home)?;
            if args.json {
                print_json(&scan_report_to_json(&report))?;
            } else {
                println!(
                    "{}",
                    report.summary.active_rollout_count + report.summary.archived_rollout_count
                );
            }
        }
        Commands::Diagnose(args) => {
            let report = scan_codex_home(&args.codex_home)?;
            let diagnosis = diagnose(&report);
            if args.json {
                print_json(&diagnosis_to_json(&diagnosis))?;
            } else {
                println!("{}", diagnosis.problems.len());
            }
        }
        Commands::Repair(args) => {
            let report = scan_codex_home(&args.codex_home)?;
            let diagnosis = diagnose(&report);
            let plan = build_repair_plan(&report, &diagnosis);
            let execution_report =
                execute_repair_plan(&args.codex_home, &args.backups_root, &plan, args.dry_run)?;
            if args.json {
                print_json(&repair_execution_report_to_json(&execution_report, &plan))?;
            } else {
                println!("{}", plan.actions.len());
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
                    println!("{}", manifests.len());
                }
            }
            BackupCommands::Restore(args) => {
                restore_backup(&args.snapshot_dir, &args.codex_home)?;
                if args.json {
                    print_json(&json!({
                        "snapshot_dir": args.snapshot_dir,
                        "codex_home": args.codex_home,
                        "restored": true,
                    }))?;
                } else {
                    println!("restored");
                }
            }
            BackupCommands::Prune(args) => {
                let report = prune_backups(&args.backups_root, args.keep_latest)?;
                if args.json {
                    print_json(&json!({
                        "removed_backup_ids": report.removed_backup_ids,
                    }))?;
                } else {
                    println!("{}", report.removed_backup_ids.len());
                }
            }
        },
    }

    Ok(())
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
            "sqlite_present": report.summary.sqlite_present,
            "sqlite_readable": report.summary.sqlite_readable,
            "active_rollout_count": report.summary.active_rollout_count,
            "archived_rollout_count": report.summary.archived_rollout_count,
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

fn repair_execution_entry_to_json(entry: &RepairExecutionEntry) -> Value {
    json!({
        "action": repair_action_to_json(&entry.action),
        "message": entry.message,
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

fn problem_code_to_str(code: &ProblemCode) -> &'static str {
    match code {
        ProblemCode::MissingSqliteThreadRow => "missing_sqlite_thread_row",
        ProblemCode::StaleSqliteRolloutPath => "stale_sqlite_rollout_path",
        ProblemCode::RolloutProviderMismatch => "rollout_provider_mismatch",
        ProblemCode::ArchivedStateMismatch => "archived_state_mismatch",
        ProblemCode::MissingRootModelProvider => "missing_root_model_provider",
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
