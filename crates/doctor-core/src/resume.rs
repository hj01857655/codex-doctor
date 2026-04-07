use std::path::{Path, PathBuf};

use crate::{ProblemCode, ScanReport, ThreadLocation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeBlocker {
    MissingSqliteThreadRow,
    Archived,
    ProviderMismatch {
        session_provider: String,
        current_provider: String,
    },
    CwdMismatch {
        session_cwd: PathBuf,
        current_cwd: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeCandidate {
    pub thread_id: String,
    pub provider: Option<String>,
    pub cwd: PathBuf,
    pub timestamp: String,
    pub location: ThreadLocation,
    pub default_picker_visible: bool,
    pub blockers: Vec<ResumeBlocker>,
    pub direct_resume_command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeCandidateScope {
    CurrentCwdOnly,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeDoctorReport {
    pub root_provider: Option<String>,
    pub candidates: Vec<ResumeCandidate>,
    pub current_cwd: PathBuf,
}

pub fn build_resume_doctor_report(
    scan_report: &ScanReport,
    current_cwd: &Path,
) -> ResumeDoctorReport {
    let mut candidates = Vec::new();

    for rollout in &scan_report.rollout_records {
        let sqlite_row = scan_report
            .sqlite_threads
            .iter()
            .find(|thread| thread.id == rollout.thread_id);

        let mut blockers = Vec::new();
        if sqlite_row.is_none() {
            blockers.push(ResumeBlocker::MissingSqliteThreadRow);
        }

        if matches!(rollout.location, ThreadLocation::Archived) || rollout.archived {
            blockers.push(ResumeBlocker::Archived);
        }

        if let (Some(current_provider), Some(session_provider)) = (
            scan_report.summary.root_provider.as_ref(),
            rollout.session_meta.provider.as_ref(),
        ) {
            if session_provider != current_provider {
                blockers.push(ResumeBlocker::ProviderMismatch {
                    session_provider: session_provider.clone(),
                    current_provider: current_provider.clone(),
                });
            }
        }

        if rollout.session_meta.cwd != current_cwd {
            blockers.push(ResumeBlocker::CwdMismatch {
                session_cwd: rollout.session_meta.cwd.clone(),
                current_cwd: current_cwd.to_path_buf(),
            });
        }

        candidates.push(ResumeCandidate {
            thread_id: rollout.thread_id.clone(),
            provider: rollout.session_meta.provider.clone(),
            cwd: rollout.session_meta.cwd.clone(),
            timestamp: rollout.session_meta.timestamp.clone(),
            location: rollout.location.clone(),
            default_picker_visible: blockers.is_empty(),
            blockers,
            direct_resume_command: sqlite_row
                .map(|_| format!("codex resume {}", rollout.thread_id)),
        });
    }

    candidates.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));

    ResumeDoctorReport {
        current_cwd: current_cwd.to_path_buf(),
        root_provider: scan_report.summary.root_provider.clone(),
        candidates,
    }
}

pub fn scoped_resume_candidates(
    report: &ResumeDoctorReport,
    scope: ResumeCandidateScope,
) -> Vec<ResumeCandidate> {
    match scope {
        ResumeCandidateScope::All => report.candidates.clone(),
        ResumeCandidateScope::CurrentCwdOnly => report
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.cwd == report.current_cwd
                    && candidate.direct_resume_command.is_some()
                    && !matches!(candidate.location, ThreadLocation::Archived)
            })
            .cloned()
            .collect(),
    }
}

pub fn diagnosis_problem_matches_resume_visibility(code: &ProblemCode) -> bool {
    matches!(
        code,
        ProblemCode::ResumePickerProviderFiltered | ProblemCode::ResumePickerArchivedFiltered
    )
}

pub fn best_resume_candidate_for_current_cwd(
    report: &ResumeDoctorReport,
) -> Option<&ResumeCandidate> {
    report.candidates.iter().find(|candidate| {
        candidate.cwd == report.current_cwd
            && candidate.direct_resume_command.is_some()
            && !matches!(candidate.location, ThreadLocation::Archived)
    })
}
