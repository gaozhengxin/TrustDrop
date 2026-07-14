use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TxStatus {
    Draft,
    Submitted,
    Confirmed,
    Reverted,
    Replaced,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxRecord {
    pub id: String,
    pub kind: String,
    pub chain_id: u64,
    pub tx_hash: Option<String>,
    pub status: TxStatus,
    pub required_confirmations: u64,
    pub block_number: Option<u64>,
    pub receipt_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub next_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    InProgress,
    Complete,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageRecord {
    pub name: String,
    pub status: StageStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleState {
    pub sale_id: String,
    pub channel_address: Option<String>,
    pub input_asset_path: Option<String>,
    pub original_len: Option<usize>,
    pub original_asset_id: Option<String>,
    pub encrypted_blob_id: Option<String>,
    pub encrypted_asset_path: Option<String>,
    pub walrus_blob_id: Option<String>,
    #[serde(default)]
    pub walrus_end_epoch: Option<u64>,
    pub data_commitment: Option<String>,
    pub data_version: Option<String>,
    pub stages: Vec<StageRecord>,
    pub transactions: Vec<TxRecord>,
    #[serde(default)]
    pub purchases: Vec<PurchaseContextRecord>,
    pub next_actions: Vec<String>,
    pub last_error: Option<String>,
}

impl SaleState {
    pub fn new(sale_id: impl Into<String>) -> Self {
        Self {
            sale_id: sale_id.into(),
            channel_address: None,
            input_asset_path: None,
            original_len: None,
            original_asset_id: None,
            encrypted_blob_id: None,
            encrypted_asset_path: None,
            walrus_blob_id: None,
            walrus_end_epoch: None,
            data_commitment: None,
            data_version: None,
            stages: Vec::new(),
            transactions: Vec::new(),
            purchases: Vec::new(),
            next_actions: Vec::new(),
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseContextRecord {
    pub purchase_tx_hash: String,
    pub buyer: Option<String>,
    pub secret_sharing_key: Option<String>,
    pub status: String,
    pub fulfill_tx_hash: Option<String>,
    pub settle_tx_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Planned,
    Ready,
    ProvingVss,
    VssReady,
    Fulfilling,
    Fulfilled,
    OraclePending,
    SettleReady,
    Settling,
    Completed,
    Blocked,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPurchase {
    pub purchase_tx_hash: String,
    pub buyer: Option<String>,
    pub sale_id: String,
    #[serde(default = "default_needs_vss")]
    pub needs_vss: bool,
    pub status: String,
    pub settle_tx_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadState {
    pub thread_id: String,
    pub channel_address: Option<String>,
    pub sale_id: String,
    pub status: ThreadStatus,
    pub purchases: Vec<ThreadPurchase>,
    pub vss_proof_id: Option<String>,
    pub fulfill_tx_hash: Option<String>,
    pub oracle_request_ids: Vec<String>,
    pub settle_tx_hashes: Vec<String>,
    pub next_actions: Vec<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ThreadState {
    pub fn new(
        thread_id: impl Into<String>,
        sale_id: impl Into<String>,
        channel_address: Option<String>,
        purchases: Vec<ThreadPurchase>,
        timestamp: impl Into<String>,
    ) -> Self {
        let timestamp = timestamp.into();
        Self {
            thread_id: thread_id.into(),
            channel_address,
            sale_id: sale_id.into(),
            status: ThreadStatus::Planned,
            purchases,
            vss_proof_id: None,
            fulfill_tx_hash: None,
            oracle_request_ids: Vec::new(),
            settle_tx_hashes: Vec::new(),
            next_actions: Vec::new(),
            last_error: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        }
    }
}

pub fn default_state_dir() -> Result<PathBuf> {
    let home = env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?;
    Ok(Path::new(&home).join(".trustdrop").join("state"))
}

pub fn state_path(state_dir: impl AsRef<Path>, sale_id: &str) -> PathBuf {
    state_dir
        .as_ref()
        .join(format!("{}.json", sanitize_sale_id(sale_id)))
}

pub fn database_path(state_dir: impl AsRef<Path>) -> PathBuf {
    state_dir.as_ref().join("trustdrop.db")
}

fn open_database(state_dir: impl AsRef<Path>) -> Result<Connection> {
    let state_dir = state_dir.as_ref();
    fs::create_dir_all(state_dir)?;
    let path = database_path(state_dir);
    let is_new = !path.exists();
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS sales (
             sale_id TEXT PRIMARY KEY,
             state_json TEXT NOT NULL,
             updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS threads (
             thread_id TEXT PRIMARY KEY,
             sale_id TEXT NOT NULL,
             state_json TEXT NOT NULL,
             updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE INDEX IF NOT EXISTS threads_sale_id ON threads(sale_id);
         CREATE TABLE IF NOT EXISTS daemon_seen_purchases (
             purchase_tx_hash TEXT PRIMARY KEY,
             first_seen_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS daemon_meta (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS daemon_events (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             created_at INTEGER NOT NULL DEFAULT (unixepoch()),
             kind TEXT NOT NULL,
             message TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS daemon_resume_queue (
             thread_id TEXT PRIMARY KEY,
             requested_at INTEGER NOT NULL DEFAULT (unixepoch())
         );",
    )?;
    if is_new {
        import_legacy_json(state_dir, &connection)?;
    }
    Ok(connection)
}

fn import_legacy_json(state_dir: &Path, connection: &Connection) -> Result<()> {
    for entry in fs::read_dir(state_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if let Ok(state) = serde_json::from_str::<SaleState>(&content) {
            connection.execute(
                "INSERT OR IGNORE INTO sales (sale_id, state_json) VALUES (?1, ?2)",
                params![state.sale_id, content],
            )?;
        }
    }
    let dir = thread_state_dir(state_dir);
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path)?;
            if let Ok(state) = serde_json::from_str::<ThreadState>(&content) {
                connection.execute(
                    "INSERT OR IGNORE INTO threads (thread_id, sale_id, state_json) VALUES (?1, ?2, ?3)",
                    params![state.thread_id, state.sale_id, content],
                )?;
            }
        }
    }
    Ok(())
}

pub fn load_sale_state(state_dir: impl AsRef<Path>, sale_id: &str) -> Result<SaleState> {
    let connection = open_database(state_dir)?;
    let content = connection
        .query_row(
            "SELECT state_json FROM sales WHERE lower(sale_id) = lower(?1)",
            [sale_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("sale state not found: {sale_id}"))?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_sale_state(state_dir: impl AsRef<Path>, state: &SaleState) -> Result<()> {
    let state_dir = state_dir.as_ref();
    let connection = open_database(state_dir)?;
    let content = serde_json::to_string(state)?;
    connection.execute(
        "INSERT INTO sales (sale_id, state_json, updated_at) VALUES (?1, ?2, unixepoch())
         ON CONFLICT(sale_id) DO UPDATE SET state_json = excluded.state_json, updated_at = excluded.updated_at",
        params![state.sale_id, content],
    )?;
    Ok(())
}

pub fn load_all_sale_states(state_dir: impl AsRef<Path>) -> Result<Vec<SaleState>> {
    let connection = open_database(state_dir)?;
    let mut statement = connection.prepare("SELECT state_json FROM sales ORDER BY sale_id")?;
    let mut states = Vec::new();
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for content in rows {
        states.push(serde_json::from_str(&content?)?);
    }
    Ok(states)
}

pub fn thread_state_dir(state_dir: impl AsRef<Path>) -> PathBuf {
    state_dir.as_ref().join("threads")
}

pub fn thread_state_path(state_dir: impl AsRef<Path>, thread_id: &str) -> PathBuf {
    thread_state_dir(state_dir).join(format!("{}.json", sanitize_sale_id(thread_id)))
}

pub fn load_thread_state(state_dir: impl AsRef<Path>, thread_id: &str) -> Result<ThreadState> {
    let connection = open_database(state_dir)?;
    let content = connection
        .query_row(
            "SELECT state_json FROM threads WHERE lower(thread_id) = lower(?1)",
            [thread_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("thread state not found: {thread_id}"))?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_thread_state(state_dir: impl AsRef<Path>, state: &ThreadState) -> Result<()> {
    let connection = open_database(state_dir)?;
    let content = serde_json::to_string(state)?;
    connection.execute(
        "INSERT INTO threads (thread_id, sale_id, state_json, updated_at) VALUES (?1, ?2, ?3, unixepoch())
         ON CONFLICT(thread_id) DO UPDATE SET sale_id = excluded.sale_id, state_json = excluded.state_json, updated_at = excluded.updated_at",
        params![state.thread_id, state.sale_id, content],
    )?;
    Ok(())
}

pub fn load_all_thread_states(state_dir: impl AsRef<Path>) -> Result<Vec<ThreadState>> {
    let connection = open_database(state_dir)?;
    let mut statement = connection.prepare("SELECT state_json FROM threads ORDER BY thread_id")?;
    let mut states = Vec::new();
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for content in rows {
        states.push(serde_json::from_str(&content?)?);
    }
    Ok(states)
}

pub fn initialize_or_load_daemon_seen(
    state_dir: impl AsRef<Path>,
    baseline: impl IntoIterator<Item = String>,
) -> Result<HashSet<String>> {
    let mut connection = open_database(state_dir)?;
    let transaction = connection.transaction()?;
    let initialized = transaction
        .query_row(
            "SELECT value FROM daemon_meta WHERE key = 'purchase_baseline_initialized'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();
    if !initialized {
        for hash in baseline {
            transaction.execute(
                "INSERT OR IGNORE INTO daemon_seen_purchases (purchase_tx_hash) VALUES (lower(?1))",
                [hash],
            )?;
        }
        transaction.execute(
            "INSERT INTO daemon_meta (key, value) VALUES ('purchase_baseline_initialized', 'true')",
            [],
        )?;
    }
    transaction.commit()?;
    load_daemon_seen(&connection)
}

fn load_daemon_seen(connection: &Connection) -> Result<HashSet<String>> {
    let mut statement = connection.prepare("SELECT purchase_tx_hash FROM daemon_seen_purchases")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut hashes = HashSet::new();
    for hash in rows {
        hashes.insert(hash?);
    }
    Ok(hashes)
}

pub fn mark_daemon_purchase_seen(state_dir: impl AsRef<Path>, hash: &str) -> Result<()> {
    open_database(state_dir)?.execute(
        "INSERT OR IGNORE INTO daemon_seen_purchases (purchase_tx_hash) VALUES (lower(?1))",
        [hash],
    )?;
    Ok(())
}

pub fn append_daemon_event(state_dir: impl AsRef<Path>, kind: &str, message: &str) -> Result<()> {
    open_database(state_dir)?.execute(
        "INSERT INTO daemon_events (kind, message) VALUES (?1, ?2)",
        params![kind, message],
    )?;
    Ok(())
}

pub fn daemon_event_count(state_dir: impl AsRef<Path>) -> Result<u64> {
    Ok(open_database(state_dir)?
        .query_row("SELECT count(*) FROM daemon_events", [], |row| row.get(0))?)
}

pub fn enqueue_thread_resume(state_dir: impl AsRef<Path>, thread_id: &str) -> Result<()> {
    open_database(state_dir)?.execute(
        "INSERT INTO daemon_resume_queue (thread_id, requested_at) VALUES (?1, unixepoch())
         ON CONFLICT(thread_id) DO UPDATE SET requested_at = excluded.requested_at",
        [thread_id],
    )?;
    Ok(())
}

pub fn load_thread_resume_queue(state_dir: impl AsRef<Path>) -> Result<Vec<String>> {
    let connection = open_database(state_dir)?;
    let mut statement = connection
        .prepare("SELECT thread_id FROM daemon_resume_queue ORDER BY requested_at, thread_id")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut thread_ids = Vec::new();
    for thread_id in rows {
        thread_ids.push(thread_id?);
    }
    Ok(thread_ids)
}

pub fn remove_thread_resume(state_dir: impl AsRef<Path>, thread_id: &str) -> Result<()> {
    open_database(state_dir)?.execute(
        "DELETE FROM daemon_resume_queue WHERE thread_id = ?1",
        [thread_id],
    )?;
    Ok(())
}

fn sanitize_sale_id(sale_id: &str) -> String {
    sale_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn default_needs_vss() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("drop-sdk-state-{name}-{nonce}"))
    }

    #[test]
    fn imports_legacy_json_then_uses_sqlite() {
        let dir = test_dir("migration");
        fs::create_dir_all(&dir).unwrap();
        let mut legacy = SaleState::new("sale-1");
        legacy.data_version = Some("legacy".into());
        fs::write(
            state_path(&dir, "sale-1"),
            serde_json::to_string(&legacy).unwrap(),
        )
        .unwrap();

        let imported = load_sale_state(&dir, "sale-1").unwrap();
        assert_eq!(imported.data_version.as_deref(), Some("legacy"));
        assert!(database_path(&dir).exists());

        legacy.data_version = Some("sqlite".into());
        save_sale_state(&dir, &legacy).unwrap();
        fs::remove_file(state_path(&dir, "sale-1")).unwrap();
        assert_eq!(
            load_sale_state(&dir, "sale-1")
                .unwrap()
                .data_version
                .as_deref(),
            Some("sqlite")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn persists_threads_and_daemon_seen_state() {
        let dir = test_dir("daemon");
        let thread = ThreadState::new(
            "thread-1",
            "sale-1",
            Some("0xchannel".into()),
            Vec::new(),
            "1",
        );
        save_thread_state(&dir, &thread).unwrap();
        assert_eq!(
            load_thread_state(&dir, "thread-1").unwrap().sale_id,
            "sale-1"
        );

        let initial = initialize_or_load_daemon_seen(&dir, vec!["0xold".into()]).unwrap();
        assert!(initial.contains("0xold"));
        let restarted =
            initialize_or_load_daemon_seen(&dir, vec!["0xnew-during-downtime".into()]).unwrap();
        assert!(restarted.contains("0xold"));
        assert!(!restarted.contains("0xnew-during-downtime"));

        mark_daemon_purchase_seen(&dir, "0xNEW-DURING-DOWNTIME").unwrap();
        let after_processing = initialize_or_load_daemon_seen(&dir, Vec::new()).unwrap();
        assert!(after_processing.contains("0xnew-during-downtime"));
        fs::remove_dir_all(dir).unwrap();
    }
}
