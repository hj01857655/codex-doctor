mod backup;
mod config;
mod diagnose;
mod history;
mod layout;
mod model;
mod plan;
mod repair;
mod resume;
mod rollout;
mod scan;
mod sqlite;

pub use backup::{
    create_backup_snapshot, create_backup_snapshot_with_sqlite_home, list_backups, prune_backups,
    restore_backup, restore_backup_with_sqlite_home, BackupManifest, BackupPruneReport,
    BackupSnapshot,
};
pub use config::{patch_root_model_provider, read_root_config_snapshot};
pub use diagnose::{diagnose, DiagnosisProblem, DiagnosisReport, ProblemCode, ProblemSeverity};
pub use history::{
    list_repair_history, save_repair_history, ActionStatus, RepairActionRecord, RepairHistoryEntry,
};
pub use layout::CodexLayout;
pub use model::{RolloutRecord, RolloutSessionMeta, RootConfigSnapshot, ThreadLocation};
pub use plan::{build_repair_plan, RepairAction, RepairPlan};
pub use repair::{
    execute_repair_plan, execute_repair_plan_with_sqlite_home, RepairExecutionEntry,
    RepairExecutionReport,
};
pub use resume::{
    best_resume_candidate_for_current_cwd, build_resume_doctor_report,
    diagnosis_problem_matches_resume_visibility, scoped_resume_candidates, ResumeBlocker,
    ResumeCandidate, ResumeCandidateScope, ResumeDoctorReport,
};
pub use rollout::{move_rollout_file, rewrite_rollout_provider};
pub use scan::{
    scan_codex_home, scan_codex_home_with_sqlite_home, ProviderDistribution, ScanReport,
    ScanSummary,
};
pub use sqlite::{
    read_thread_by_id, read_threads, upsert_thread_record, SqliteReaderError, SqliteThreadRecord,
};

pub fn version_banner() -> &'static str {
    "codex-doctor core"
}
