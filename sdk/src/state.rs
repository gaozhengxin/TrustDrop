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
