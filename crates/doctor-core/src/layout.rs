use std::env;
use std::path::{Path, PathBuf};

const STATE_DB_FILENAME: &str = "state_5.sqlite";
const LOGS_DB_FILENAME: &str = "logs_2.sqlite";
const SQLITE_HOME_ENV: &str = "CODEX_SQLITE_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexLayout {
    pub codex_home: PathBuf,
    pub config_toml: PathBuf,
    pub sessions_dir: PathBuf,
    pub archived_sessions_dir: PathBuf,
    pub state_db: PathBuf,
    pub logs_db: PathBuf,
    pub history_jsonl: PathBuf,
    pub sqlite_home: PathBuf,
}

impl CodexLayout {
    pub fn from_codex_home<P>(codex_home: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self::from_codex_home_and_env(codex_home, env::var_os(SQLITE_HOME_ENV).map(PathBuf::from))
    }

    pub fn from_codex_home_and_env<P>(codex_home: P, sqlite_home_override: Option<PathBuf>) -> Self
    where
        P: AsRef<Path>,
    {
        let codex_home = codex_home.as_ref().to_path_buf();
        let sqlite_home = sqlite_home_override.unwrap_or_else(|| codex_home.clone());

        Self {
            config_toml: codex_home.join("config.toml"),
            sessions_dir: codex_home.join("sessions"),
            archived_sessions_dir: codex_home.join("archived_sessions"),
            state_db: sqlite_home.join(STATE_DB_FILENAME),
            logs_db: sqlite_home.join(LOGS_DB_FILENAME),
            history_jsonl: codex_home.join("history.jsonl"),
            sqlite_home,
            codex_home,
        }
    }
}
