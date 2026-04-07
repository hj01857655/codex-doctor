use doctor_core::{
    BackupManifest, DiagnosisProblem, ProblemSeverity, RepairActionRecord, RepairHistoryEntry,
    ResumeBlocker, ResumeDoctorReport, RolloutRecord, ScanReport, SqliteThreadRecord,
};

pub fn print_scan_report_human(report: &ScanReport) {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    Codex Doctor - Scan Report                ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    println!("📊 Summary:");
    println!(
        "  ├─ Config present:      {}",
        yes_no(report.summary.config_present)
    );
    println!(
        "  ├─ Sessions present:    {}",
        yes_no(report.summary.sessions_present)
    );
    println!(
        "  ├─ SQLite present:      {}",
        yes_no(report.summary.sqlite_present)
    );
    println!(
        "  ├─ SQLite readable:     {}",
        yes_no(report.summary.sqlite_readable)
    );
    println!(
        "  ├─ SQLite locked:       {}",
        yes_no(report.summary.sqlite_locked)
    );
    println!(
        "  ├─ Logs present:        {}",
        yes_no(report.summary.logs_present)
    );
    println!(
        "  ├─ Logs readable:       {}",
        yes_no(report.summary.logs_readable)
    );
    println!(
        "  ├─ History present:     {}",
        yes_no(report.summary.history_present)
    );
    println!(
        "  ├─ History readable:    {}",
        yes_no(report.summary.history_readable)
    );
    println!(
        "  ├─ Active sessions:     {}",
        report.summary.active_rollout_count
    );
    println!(
        "  ├─ Archived sessions:   {}",
        report.summary.archived_rollout_count
    );
    println!(
        "  ├─ Locked rollouts:     {}",
        report.summary.locked_rollout_count
    );
    println!(
        "  └─ Root provider:       {}",
        report
            .summary
            .root_provider
            .as_deref()
            .unwrap_or("(not set)")
    );
    println!();

    if !report.providers.rollout.is_empty() || !report.providers.sqlite.is_empty() {
        println!("🔧 Provider Distribution:");
        if !report.providers.rollout.is_empty() {
            println!("  Rollout files:");
            for (provider, count) in &report.providers.rollout {
                println!("    ├─ {}: {}", provider, count);
            }
        }
        if !report.providers.sqlite.is_empty() {
            println!("  SQLite threads:");
            for (provider, count) in &report.providers.sqlite {
                println!("    ├─ {}: {}", provider, count);
            }
        }
        println!();
    }

    if !report.rollout_records.is_empty() {
        println!("📁 Rollout Records: (showing first 5)");
        for (i, record) in report.rollout_records.iter().take(5).enumerate() {
            print_rollout_record(record, i == report.rollout_records.len().min(5) - 1);
        }
        if report.rollout_records.len() > 5 {
            println!("  ... and {} more", report.rollout_records.len() - 5);
        }
        println!();
    }

    if !report.sqlite_threads.is_empty() {
        println!("💾 SQLite Threads: (showing first 5)");
        for (i, thread) in report.sqlite_threads.iter().take(5).enumerate() {
            print_sqlite_thread(thread, i == report.sqlite_threads.len().min(5) - 1);
        }
        if report.sqlite_threads.len() > 5 {
            println!("  ... and {} more", report.sqlite_threads.len() - 5);
        }
        println!();
    }
}

pub fn print_diagnosis_report_human(problems: &[DiagnosisProblem]) {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                 Codex Doctor - Diagnosis Report              ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    if problems.is_empty() {
        println!("✅ No problems detected!");
        println!();
        return;
    }

    println!("⚠️  Found {} problem(s):", problems.len());
    println!();

    for (i, problem) in problems.iter().enumerate() {
        let is_last = i == problems.len() - 1;
        print_problem(problem, is_last);
    }
    println!();
}

pub fn print_repair_execution_human(
    applied: usize,
    skipped: usize,
    failed: usize,
    backup_id: Option<&str>,
    retryable_hint: Option<&str>,
) {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                Codex Doctor - Repair Execution               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    if let Some(id) = backup_id {
        println!("💾 Backup created: {}", id);
        println!();
    }

    println!("📊 Execution Summary:");
    println!("  ├─ Applied:  {} action(s)", applied);
    println!("  ├─ Skipped:  {} action(s)", skipped);
    println!("  └─ Failed:   {} action(s)", failed);
    println!();

    if failed > 0 {
        println!("❌ Some actions failed. Check the detailed output above.");
        if let Some(hint) = retryable_hint {
            println!("↻ Next step: {hint}");
        }
    } else if applied > 0 {
        println!("✅ Repair completed successfully!");
    } else {
        println!("ℹ️  No actions were needed.");
    }
    println!();
}

pub fn print_backup_list_human(manifests: &[BackupManifest]) {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                  Codex Doctor - Backup List                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    if manifests.is_empty() {
        println!("ℹ️  No backups found.");
        println!();
        return;
    }

    println!("💾 Found {} backup(s):", manifests.len());
    println!();

    for (i, manifest) in manifests.iter().enumerate() {
        let is_last = i == manifests.len() - 1;
        print_backup_manifest(manifest, is_last);
    }
    println!();
}

pub fn print_repair_history_human(entries: &[RepairHistoryEntry]) {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                 Codex Doctor - Repair History                ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    if entries.is_empty() {
        println!("ℹ️  No repair history found.");
        println!();
        return;
    }

    println!("📜 Found {} repair(s):", entries.len());
    println!();

    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        print_repair_history_entry(entry, is_last);
    }
    println!();
}

pub fn print_resume_doctor_human(report: &ResumeDoctorReport) {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                 Codex Doctor - Resume Doctor                 ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Current cwd: {}", report.current_cwd.display());
    println!(
        "Root provider: {}",
        report.root_provider.as_deref().unwrap_or("(not set)")
    );
    println!("Scope: current cwd");
    println!("Candidates (newest first): {}", report.candidates.len());
    println!();

    if report.candidates.is_empty() {
        println!(
            "ℹ️  No default-visible current-cwd sessions found. Use --all to inspect hidden or other-directory sessions."
        );
        println!();
        return;
    }

    println!(
        "{:<3} {:<20} {:<10} {:<8} {:<12} Why hidden",
        "#", "Updated", "Provider", "Loc", "Thread"
    );
    println!(
        "{:-<3} {:-<20} {:-<10} {:-<8} {:-<12} {:-<1}",
        "", "", "", "", "", ""
    );

    for (index, candidate) in report.candidates.iter().enumerate() {
        let location = match candidate.location {
            doctor_core::ThreadLocation::Active => "active",
            doctor_core::ThreadLocation::Archived => "archived",
        };
        let hidden_reason = if candidate.blockers.is_empty() {
            "-".to_string()
        } else {
            resume_blocker_summary(&candidate.blockers)
        };

        println!(
            "{:<3} {:<20} {:<10} {:<8} {:<12} {}",
            index + 1,
            format_resume_timestamp(&candidate.timestamp),
            truncate_cell(candidate.provider.as_deref().unwrap_or("(unknown)"), 10),
            location,
            truncate_cell(&candidate.thread_id, 12),
            truncate_cell(&hidden_reason, 48)
        );
    }

    println!();
    if let Some((index, candidate)) = report.candidates.iter().enumerate().find(|(_, candidate)| {
        candidate.cwd == report.current_cwd
            && candidate.default_picker_visible
            && candidate.direct_resume_command.is_some()
    }) {
        println!("Recommended: #{} {}", index + 1, candidate.thread_id);
        if let Some(command) = &candidate.direct_resume_command {
            println!("Command: {}", command);
        }
        println!();
    }

    println!(
        "Enter a candidate number to run `codex resume <thread_id>`, or press Enter to cancel."
    );
    println!();
}

fn resume_blocker_summary(blockers: &[ResumeBlocker]) -> String {
    blockers
        .iter()
        .map(|blocker| match blocker {
            ResumeBlocker::MissingSqliteThreadRow => "missing sqlite row".to_string(),
            ResumeBlocker::Archived => "archived".to_string(),
            ResumeBlocker::ProviderMismatch { .. } => "provider mismatch".to_string(),
            ResumeBlocker::CwdMismatch { .. } => "cwd mismatch".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn truncate_cell(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        value.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        format!("{}…", chars[..max_chars - 1].iter().collect::<String>())
    }
}

fn format_resume_timestamp(timestamp: &str) -> String {
    let parsed = chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.with_timezone(&chrono::Local))
        .ok();
    let Some(dt) = parsed else {
        return truncate_cell(timestamp, 20);
    };

    let now = chrono::Local::now();
    let delta = now.signed_duration_since(dt);
    if delta.num_minutes() < 1 {
        "just now".to_string()
    } else if delta.num_hours() < 1 {
        format!("{} min ago", delta.num_minutes())
    } else if delta.num_days() < 1 {
        format!("{} hr ago", delta.num_hours())
    } else if delta.num_days() < 7 {
        format!("{} day ago", delta.num_days())
    } else {
        dt.format("%m-%d %H:%M").to_string()
    }
}

fn print_rollout_record(record: &RolloutRecord, is_last: bool) {
    let prefix = if is_last { "└─" } else { "├─" };
    println!("  {} Thread: {}", prefix, &record.thread_id[..8]);
    println!(
        "     Provider: {}",
        record
            .session_meta
            .provider
            .as_deref()
            .unwrap_or("(unknown)")
    );
    println!("     Location: {:?}", record.location);
}

fn print_sqlite_thread(thread: &SqliteThreadRecord, is_last: bool) {
    let prefix = if is_last { "└─" } else { "├─" };
    println!("  {} Thread: {}", prefix, &thread.id[..8]);
    println!("     Provider: {}", thread.model_provider);
    println!(
        "     Archived: {}",
        if thread.archived_at.is_some() {
            "yes"
        } else {
            "no"
        }
    );
}

fn print_problem(problem: &DiagnosisProblem, is_last: bool) {
    let prefix = if is_last { "└─" } else { "├─" };
    let severity_icon = match problem.severity {
        ProblemSeverity::Error => "❌",
        ProblemSeverity::Warning => "⚠️ ",
        ProblemSeverity::Info => "ℹ️ ",
    };

    println!("  {} {} {:?}", prefix, severity_icon, problem.code);
    for evidence in &problem.evidence {
        println!("     Evidence: {}", evidence);
    }
    if !problem.suggested_fix_ids.is_empty() {
        println!(
            "     Suggested fixes: {}",
            problem.suggested_fix_ids.join(", ")
        );
    }
}

fn print_backup_manifest(manifest: &BackupManifest, is_last: bool) {
    let prefix = if is_last { "└─" } else { "├─" };
    println!("  {} Backup ID: {}", prefix, manifest.backup_id);
    println!("     Source: {}", manifest.source_codex_home.display());
    println!(
        "     Created: {}",
        format_timestamp(manifest.created_at_unix_ms as i64)
    );
}

fn print_repair_history_entry(entry: &RepairHistoryEntry, is_last: bool) {
    let prefix = if is_last { "└─" } else { "├─" };
    println!(
        "  {} Timestamp: {}",
        prefix,
        format_timestamp(entry.timestamp * 1000)
    );
    println!("     Codex home: {}", entry.codex_home.display());
    println!(
        "     Actions: {} applied, {} skipped, {} failed",
        entry.actions_applied, entry.actions_skipped, entry.actions_failed
    );
    if let Some(backup_id) = &entry.backup_id {
        println!("     Backup: {}", backup_id);
    }
    if !entry.actions.is_empty() {
        println!("     Details:");
        for action in &entry.actions {
            print_repair_action_record(action);
        }
    }
}

fn print_repair_action_record(action: &RepairActionRecord) {
    let status_icon = match action.status {
        doctor_core::ActionStatus::Applied => "✓",
        doctor_core::ActionStatus::Skipped => "○",
        doctor_core::ActionStatus::Failed => "✗",
    };
    let retryable_suffix = if action.retryable { " (retryable)" } else { "" };
    println!(
        "       {} {}{} - {}",
        status_icon, action.action_type, retryable_suffix, action.details
    );
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn format_timestamp(millis: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let duration = Duration::from_millis(millis as u64);
    let datetime = UNIX_EPOCH + duration;
    let datetime: chrono::DateTime<chrono::Local> = datetime.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}
