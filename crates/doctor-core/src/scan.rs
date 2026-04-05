use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    read_root_config_snapshot, read_threads, CodexLayout, RolloutRecord, RootConfigSnapshot,
    SqliteThreadRecord, ThreadLocation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    pub config_present: bool,
    pub sqlite_present: bool,
    pub sqlite_readable: bool,
    pub active_rollout_count: usize,
    pub archived_rollout_count: usize,
    pub root_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderDistribution {
    pub rollout: BTreeMap<String, usize>,
    pub sqlite: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub summary: ScanSummary,
    pub providers: ProviderDistribution,
    pub root_config: Option<RootConfigSnapshot>,
    pub rollout_records: Vec<RolloutRecord>,
    pub sqlite_threads: Vec<SqliteThreadRecord>,
}

pub fn scan_codex_home(codex_home: &Path) -> Result<ScanReport, String> {
    let layout = CodexLayout::from_codex_home(codex_home);

    let config_present = layout.config_toml.exists();
    let sqlite_present = layout.state_db.exists();

    let root_config = if config_present {
        Some(read_root_config_snapshot(&layout.config_toml)?)
    } else {
        None
    };
    let root_provider = root_config
        .as_ref()
        .and_then(|config| config.model_provider.clone());

    let active_rollouts = read_rollouts_in_dir(&layout.sessions_dir, ThreadLocation::Active)?;
    let archived_rollouts =
        read_rollouts_in_dir(&layout.archived_sessions_dir, ThreadLocation::Archived)?;

    let mut rollout_distribution = BTreeMap::new();
    for record in active_rollouts.iter().chain(archived_rollouts.iter()) {
        if let Some(provider) = record.session_meta.provider.as_ref() {
            *rollout_distribution.entry(provider.clone()).or_insert(0) += 1;
        }
    }

    let (sqlite_readable, sqlite_distribution, sqlite_threads) = if sqlite_present {
        match read_threads(&layout.state_db) {
            Ok(rows) => {
                let mut distribution = BTreeMap::new();
                for row in &rows {
                    *distribution.entry(row.model_provider.clone()).or_insert(0) += 1;
                }
                (true, distribution, rows)
            }
            Err(_) => (false, BTreeMap::new(), Vec::new()),
        }
    } else {
        (false, BTreeMap::new(), Vec::new())
    };

    Ok(ScanReport {
        summary: ScanSummary {
            config_present,
            sqlite_present,
            sqlite_readable,
            active_rollout_count: active_rollouts.len(),
            archived_rollout_count: archived_rollouts.len(),
            root_provider,
        },
        providers: ProviderDistribution {
            rollout: rollout_distribution,
            sqlite: sqlite_distribution,
        },
        root_config,
        rollout_records: active_rollouts
            .into_iter()
            .chain(archived_rollouts)
            .collect(),
        sqlite_threads,
    })
}

fn read_rollouts_in_dir(
    dir: &Path,
    location: ThreadLocation,
) -> Result<Vec<RolloutRecord>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            records.push(RolloutRecord::from_path(&path, location.clone())?);
        }
    }

    Ok(records)
}
