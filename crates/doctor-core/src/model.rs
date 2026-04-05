use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadLocation {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutSessionMeta {
    pub provider: Option<String>,
    pub cwd: PathBuf,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutRecord {
    pub thread_id: String,
    pub rollout_path: PathBuf,
    pub session_meta: RolloutSessionMeta,
    pub location: ThreadLocation,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootConfigSnapshot {
    pub model_provider: Option<String>,
}
