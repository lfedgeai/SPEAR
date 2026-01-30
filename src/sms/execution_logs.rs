use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use uuid::Uuid;

const EXECUTION_LOGS_DIR: &str = "./data/execution_logs";
const LOGS_FILE_NAME: &str = "logs.ndjson";
const META_FILE_NAME: &str = "meta.json";

static EXECUTION_LOCKS: OnceLock<DashMap<String, std::sync::Arc<Mutex<()>>>> = OnceLock::new();

fn execution_locks() -> &'static DashMap<String, std::sync::Arc<Mutex<()>>> {
    EXECUTION_LOCKS.get_or_init(DashMap::new)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredLogLine {
    pub ts_ms: u64,
    pub seq: u64,
    pub stream: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMeta {
    pub execution_id: String,
    pub next_seq: u64,
    pub total_bytes: u64,
    pub truncated: bool,
    pub completed: bool,
    pub updated_at_ms: u64,
}

impl LogMeta {
    fn new(execution_id: String, now_ms: u64) -> Self {
        Self {
            execution_id,
            next_seq: 1,
            total_bytes: 0,
            truncated: false,
            completed: false,
            updated_at_ms: now_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendLogLine {
    pub ts_ms: Option<u64>,
    pub stream: Option<String>,
    pub level: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AppendResult {
    pub accepted: usize,
    pub truncated: bool,
    pub next_seq: u64,
}

#[derive(Debug, Clone)]
pub struct AppendWithSeqResult {
    pub accepted: u64,
    pub acked_seq: u64,
    pub truncated: bool,
    pub next_seq: u64,
}

#[derive(Debug, Clone)]
pub enum AppendWithSeqError {
    InvalidExecutionId,
    Completed,
    Truncated,
    InvalidSeq { seq: u64 },
    OutOfOrder { expected: u64, got: u64 },
    Io(String),
}

impl From<std::io::Error> for AppendWithSeqError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ReadResult {
    pub lines: Vec<StoredLogLine>,
    pub next_cursor: String,
    pub truncated: bool,
    pub completed: bool,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn max_bytes_per_execution() -> u64 {
    std::env::var("SMS_EXECUTION_LOG_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(10 * 1024 * 1024)
}

fn sanitize_execution_id(id: &str) -> Option<&str> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return None;
    }
    Some(id)
}

fn exec_dir(execution_id: &str) -> PathBuf {
    Path::new(EXECUTION_LOGS_DIR).join(execution_id)
}

fn logs_path(execution_id: &str) -> PathBuf {
    exec_dir(execution_id).join(LOGS_FILE_NAME)
}

fn meta_path(execution_id: &str) -> PathBuf {
    exec_dir(execution_id).join(META_FILE_NAME)
}

async fn load_or_init_meta(execution_id: &str) -> Result<LogMeta, std::io::Error> {
    let p = meta_path(execution_id);
    match tokio::fs::read(&p).await {
        Ok(bytes) => {
            let meta = serde_json::from_slice::<LogMeta>(&bytes)
                .unwrap_or_else(|_| LogMeta::new(execution_id.to_string(), now_ms()));
            Ok(meta)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(LogMeta::new(execution_id.to_string(), now_ms()))
        }
        Err(e) => Err(e),
    }
}

async fn save_meta(meta: &LogMeta) -> Result<(), std::io::Error> {
    let dir = exec_dir(&meta.execution_id);
    tokio::fs::create_dir_all(&dir).await?;
    let tmp = dir.join(format!("{}.{}.tmp", META_FILE_NAME, Uuid::new_v4()));
    let bytes = serde_json::to_vec(meta).unwrap_or_else(|_| b"{}".to_vec());
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, meta_path(&meta.execution_id)).await?;
    Ok(())
}

pub async fn append_logs(
    execution_id: &str,
    lines: Vec<AppendLogLine>,
) -> Result<AppendResult, std::io::Error> {
    let Some(execution_id) = sanitize_execution_id(execution_id) else {
        return Ok(AppendResult {
            accepted: 0,
            truncated: false,
            next_seq: 1,
        });
    };

    let lock = execution_locks()
        .entry(execution_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone();

    let _g = lock.lock().await;

    let dir = exec_dir(execution_id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut meta = load_or_init_meta(execution_id).await?;

    if meta.completed {
        return Ok(AppendResult {
            accepted: 0,
            truncated: meta.truncated,
            next_seq: meta.next_seq,
        });
    }

    let max_bytes = max_bytes_per_execution();
    if meta.truncated || meta.total_bytes >= max_bytes {
        meta.truncated = true;
        meta.updated_at_ms = now_ms();
        let _ = save_meta(&meta).await;
        return Ok(AppendResult {
            accepted: 0,
            truncated: true,
            next_seq: meta.next_seq,
        });
    }

    let mut accepted = 0usize;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_path(execution_id))
        .await?;

    for l in lines {
        if meta.total_bytes >= max_bytes {
            meta.truncated = true;
            break;
        }
        let stored = StoredLogLine {
            ts_ms: l.ts_ms.unwrap_or_else(now_ms),
            seq: meta.next_seq,
            stream: l.stream.unwrap_or_else(|| "stdout".to_string()),
            level: l.level.unwrap_or_else(|| "info".to_string()),
            message: l.message,
        };
        let mut line_bytes = serde_json::to_vec(&stored).unwrap_or_else(|_| b"{}".to_vec());
        line_bytes.push(b'\n');
        file.write_all(&line_bytes).await?;
        meta.total_bytes = meta.total_bytes.saturating_add(line_bytes.len() as u64);
        meta.next_seq = meta.next_seq.saturating_add(1);
        accepted += 1;
    }

    meta.updated_at_ms = now_ms();
    let _ = save_meta(&meta).await;

    Ok(AppendResult {
        accepted,
        truncated: meta.truncated,
        next_seq: meta.next_seq,
    })
}

pub async fn append_logs_with_seq(
    execution_id: &str,
    lines: Vec<StoredLogLine>,
) -> Result<AppendWithSeqResult, AppendWithSeqError> {
    let Some(execution_id) = sanitize_execution_id(execution_id) else {
        return Err(AppendWithSeqError::InvalidExecutionId);
    };

    let lock = execution_locks()
        .entry(execution_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone();

    let _g = lock.lock().await;

    let dir = exec_dir(execution_id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut meta = load_or_init_meta(execution_id).await?;

    if meta.completed {
        return Err(AppendWithSeqError::Completed);
    }

    let max_bytes = max_bytes_per_execution();
    if meta.truncated || meta.total_bytes >= max_bytes {
        meta.truncated = true;
        meta.updated_at_ms = now_ms();
        let _ = save_meta(&meta).await;
        return Err(AppendWithSeqError::Truncated);
    }

    let mut accepted: u64 = 0;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_path(execution_id))
        .await?;

    for mut l in lines {
        if meta.total_bytes >= max_bytes {
            meta.truncated = true;
            break;
        }
        if l.seq == 0 {
            return Err(AppendWithSeqError::InvalidSeq { seq: 0 });
        }
        if l.seq < meta.next_seq {
            continue;
        }
        if l.seq != meta.next_seq {
            return Err(AppendWithSeqError::OutOfOrder {
                expected: meta.next_seq,
                got: l.seq,
            });
        }
        if l.stream.trim().is_empty() {
            l.stream = "stdout".to_string();
        }
        if l.level.trim().is_empty() {
            l.level = "info".to_string();
        }

        let mut line_bytes = serde_json::to_vec(&l).unwrap_or_else(|_| b"{}".to_vec());
        line_bytes.push(b'\n');
        file.write_all(&line_bytes).await?;
        meta.total_bytes = meta.total_bytes.saturating_add(line_bytes.len() as u64);
        meta.next_seq = meta.next_seq.saturating_add(1);
        accepted = accepted.saturating_add(1);
    }

    meta.updated_at_ms = now_ms();
    let _ = save_meta(&meta).await;

    Ok(AppendWithSeqResult {
        accepted,
        acked_seq: meta.next_seq.saturating_sub(1),
        truncated: meta.truncated,
        next_seq: meta.next_seq,
    })
}

pub async fn finalize_execution_logs(execution_id: &str) -> Result<LogMeta, std::io::Error> {
    let Some(execution_id) = sanitize_execution_id(execution_id) else {
        return Ok(LogMeta::new("".to_string(), now_ms()));
    };

    let lock = execution_locks()
        .entry(execution_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone();
    let _g = lock.lock().await;

    let mut meta = load_or_init_meta(execution_id).await?;
    meta.completed = true;
    meta.updated_at_ms = now_ms();
    let _ = save_meta(&meta).await;
    Ok(meta)
}

fn parse_cursor_seq(cursor: Option<&str>) -> u64 {
    cursor.unwrap_or("").trim().parse::<u64>().unwrap_or(0)
}

pub async fn read_logs_page(
    execution_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<ReadResult, std::io::Error> {
    let Some(execution_id) = sanitize_execution_id(execution_id) else {
        return Ok(ReadResult {
            lines: Vec::new(),
            next_cursor: "0".to_string(),
            truncated: false,
            completed: false,
        });
    };

    let meta = load_or_init_meta(execution_id)
        .await
        .unwrap_or_else(|_| LogMeta::new(execution_id.to_string(), now_ms()));

    let start_after_seq = parse_cursor_seq(cursor);
    let mut out = Vec::new();
    let p = logs_path(execution_id);
    let f = match tokio::fs::File::open(&p).await {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ReadResult {
                lines: Vec::new(),
                next_cursor: start_after_seq.to_string(),
                truncated: meta.truncated,
                completed: meta.completed,
            });
        }
        Err(e) => return Err(e),
    };

    let mut reader = tokio::io::BufReader::new(f);
    let mut buf = String::new();
    while out.len() < limit {
        buf.clear();
        let n = reader.read_line(&mut buf).await?;
        if n == 0 {
            break;
        }
        let line = buf.trim_end();
        if line.is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<StoredLogLine>(line);
        let Ok(v) = parsed else {
            continue;
        };
        if v.seq <= start_after_seq {
            continue;
        }
        out.push(v);
    }

    let next_cursor = out
        .last()
        .map(|l| l.seq.to_string())
        .unwrap_or_else(|| start_after_seq.to_string());

    Ok(ReadResult {
        lines: out,
        next_cursor,
        truncated: meta.truncated,
        completed: meta.completed,
    })
}

pub async fn read_logs_download_text(
    execution_id: &str,
) -> Result<(Vec<u8>, bool), std::io::Error> {
    let Some(execution_id) = sanitize_execution_id(execution_id) else {
        return Ok((Vec::new(), false));
    };
    let meta = load_or_init_meta(execution_id)
        .await
        .unwrap_or_else(|_| LogMeta::new(execution_id.to_string(), now_ms()));
    let p = logs_path(execution_id);
    let bytes = match tokio::fs::read(&p).await {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };

    let mut out = Vec::with_capacity(bytes.len());
    for line in bytes.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let parsed = serde_json::from_slice::<StoredLogLine>(line);
        let Ok(v) = parsed else {
            continue;
        };
        let s = format!("{}\t{}\t{}\t{}\n", v.ts_ms, v.stream, v.level, v.message);
        out.extend_from_slice(s.as_bytes());
    }

    Ok((out, meta.truncated))
}
