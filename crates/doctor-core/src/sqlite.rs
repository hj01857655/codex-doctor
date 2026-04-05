use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OpenFlags};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteThreadRecord {
    pub id: String,
    pub rollout_path: PathBuf,
    pub model_provider: String,
    pub archived_at: Option<i64>,
    pub cwd: PathBuf,
}

#[derive(Debug)]
pub enum SqliteReaderError {
    Open { path: PathBuf, message: String },
    Query { message: String },
}

impl std::fmt::Display for SqliteReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open { path, message } => {
                write!(f, "failed to open sqlite database {}: {message}", path.display())
            }
            Self::Query { message } => write!(f, "failed to query sqlite database: {message}"),
        }
    }
}

impl std::error::Error for SqliteReaderError {}

pub fn read_threads(path: &Path) -> Result<Vec<SqliteThreadRecord>, SqliteReaderError> {
    let connection = open_read_only(path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT id, rollout_path, model_provider, archived_at, cwd
            FROM threads
            ORDER BY updated_at DESC, id ASC
            ",
        )
        .map_err(|err| SqliteReaderError::Query {
            message: err.to_string(),
        })?;

    let rows = statement
        .query_map([], |row| Ok(map_row(row)?))
        .map_err(|err| SqliteReaderError::Query {
            message: err.to_string(),
        })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| SqliteReaderError::Query {
            message: err.to_string(),
        })
}

pub fn read_thread_by_id(
    path: &Path,
    thread_id: &str,
) -> Result<Option<SqliteThreadRecord>, SqliteReaderError> {
    let connection = open_read_only(path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT id, rollout_path, model_provider, archived_at, cwd
            FROM threads
            WHERE id = ?1
            ",
        )
        .map_err(|err| SqliteReaderError::Query {
            message: err.to_string(),
        })?;

    let mut rows = statement
        .query([thread_id])
        .map_err(|err| SqliteReaderError::Query {
            message: err.to_string(),
        })?;

    let Some(row) = rows.next().map_err(|err| SqliteReaderError::Query {
        message: err.to_string(),
    })? else {
        return Ok(None);
    };

    map_row(&row)
        .map(Some)
        .map_err(|err| SqliteReaderError::Query {
            message: err.to_string(),
        })
}

pub fn upsert_thread_record(path: &Path, record: &SqliteThreadRecord) -> Result<(), String> {
    let connection = Connection::open(path).map_err(|err| err.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_secs() as i64;

    connection
        .execute(
            "
            INSERT INTO threads (
                id, rollout_path, created_at, updated_at, source, agent_nickname, agent_role,
                agent_path, model_provider, model, reasoning_effort, cwd, cli_version, title,
                sandbox_policy, approval_mode, tokens_used, first_user_message, archived_at,
                git_sha, git_branch, git_origin_url
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL, ?6, NULL, NULL, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL, NULL, NULL)
            ON CONFLICT(id) DO UPDATE SET
                rollout_path = excluded.rollout_path,
                updated_at = excluded.updated_at,
                model_provider = excluded.model_provider,
                cwd = excluded.cwd,
                archived_at = excluded.archived_at
            ",
            params![
                record.id,
                record.rollout_path.display().to_string(),
                now,
                now,
                "doctor-core",
                record.model_provider,
                record.cwd.display().to_string(),
                "0.1.0",
                "Recovered thread",
                "read-only",
                "on-request",
                0_i64,
                "repaired by codex-doctor",
                record.archived_at,
            ],
        )
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SqliteThreadRecord> {
    Ok(SqliteThreadRecord {
        id: row.get("id")?,
        rollout_path: PathBuf::from(row.get::<_, String>("rollout_path")?),
        model_provider: row.get("model_provider")?,
        archived_at: row.get("archived_at")?,
        cwd: PathBuf::from(row.get::<_, String>("cwd")?),
    })
}

fn open_read_only(path: &Path) -> Result<Connection, SqliteReaderError> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|err| {
        SqliteReaderError::Open {
            path: path.to_path_buf(),
            message: err.to_string(),
        }
    })
}
