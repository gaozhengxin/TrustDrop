use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{
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
    pub data_commitment: Option<String>,
    pub data_version: Option<String>,
    pub stages: Vec<StageRecord>,
    pub transactions: Vec<TxRecord>,
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
            data_commitment: None,
            data_version: None,
            stages: Vec::new(),
            transactions: Vec::new(),
            next_actions: Vec::new(),
            last_error: None,
        }
    }
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

pub fn load_sale_state(state_dir: impl AsRef<Path>, sale_id: &str) -> Result<SaleState> {
    let path = state_path(state_dir, sale_id);
    let content = fs::read_to_string(&path)
        .map_err(|error| anyhow!("failed to read {}: {}", path.display(), error))?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_sale_state(state_dir: impl AsRef<Path>, state: &SaleState) -> Result<()> {
    let state_dir = state_dir.as_ref();
    fs::create_dir_all(state_dir)?;
    let path = state_path(state_dir, &state.sale_id);
    let content = serde_json::to_string_pretty(state)?;
    fs::write(path, format!("{}\n", content))?;
    Ok(())
}

pub fn load_all_sale_states(state_dir: impl AsRef<Path>) -> Result<Vec<SaleState>> {
    let state_dir = state_dir.as_ref();
    if !state_dir.exists() {
        return Ok(Vec::new());
    }

    let mut states = Vec::new();
    for entry in fs::read_dir(state_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if let Ok(state) = serde_json::from_str::<SaleState>(&content) {
            states.push(state);
        }
    }
    states.sort_by(|left, right| left.sale_id.cmp(&right.sale_id));
    Ok(states)
}

pub fn thread_state_dir(state_dir: impl AsRef<Path>) -> PathBuf {
    state_dir.as_ref().join("threads")
}

pub fn thread_state_path(state_dir: impl AsRef<Path>, thread_id: &str) -> PathBuf {
    thread_state_dir(state_dir).join(format!("{}.json", sanitize_sale_id(thread_id)))
}

pub fn load_thread_state(state_dir: impl AsRef<Path>, thread_id: &str) -> Result<ThreadState> {
    let path = thread_state_path(state_dir, thread_id);
    let content = fs::read_to_string(&path)
        .map_err(|error| anyhow!("failed to read {}: {}", path.display(), error))?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_thread_state(state_dir: impl AsRef<Path>, state: &ThreadState) -> Result<()> {
    let dir = thread_state_dir(&state_dir);
    fs::create_dir_all(&dir)?;
    let path = thread_state_path(state_dir, &state.thread_id);
    let content = serde_json::to_string_pretty(state)?;
    fs::write(path, format!("{}\n", content))?;
    Ok(())
}

pub fn load_all_thread_states(state_dir: impl AsRef<Path>) -> Result<Vec<ThreadState>> {
    let dir = thread_state_dir(state_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut states = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if let Ok(state) = serde_json::from_str::<ThreadState>(&content) {
            states.push(state);
        }
    }
    states.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
    Ok(states)
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
