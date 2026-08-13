use crate::key_manager::derive_asset_encryption_key_from_seller_key;
use anyhow::{Result, anyhow};
use std::{collections::BTreeMap, env, fs, path::Path};

#[derive(Debug, Clone, Default)]
pub struct DropCliConfig {
    pub rpc_url: Option<String>,
    pub chain_id: u64,
    pub seller_private_key: Option<String>,
    pub sp1_private_key: Option<String>,
    pub buyer_private_key: Option<String>,
    pub hub_address: Option<String>,
    pub vss_verifier_address: Option<String>,
    pub vdd_verifier_address: Option<String>,
    pub subgraph_query_url: Option<String>,
    pub oracle_worker_url: Option<String>,
    pub oracle_worker_token: Option<String>,
    pub walrus_publisher_url: Option<String>,
    pub walrus_aggregator_url: Option<String>,
    pub state_dir: Option<String>,
    pub owner_secret_key: Option<[u8; 32]>,
    pub base_env_path: Option<String>,
}

impl DropCliConfig {
    pub fn from_env_file(path: impl AsRef<Path>) -> Result<Self> {
        let vars = parse_env_with_base(path)?;
        Ok(Self {
            rpc_url: first_value(&vars, &["ARBITRUM_SEPOLIA_RPC_URL", "ARBITRUM_SEPOLIA_RPC"]),
            chain_id: first_value(&vars, &["CHAIN_ID"])
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(421614),
            seller_private_key: first_value(&vars, &["SELLER_KEY", "PRIVATE_KEY"]),
            sp1_private_key: first_value(&vars, &["SP1_PRIVATE_KEY", "NETWORK_PRIVATE_KEY"]),
            buyer_private_key: first_value(&vars, &["BUYER_KEY"]),
            hub_address: first_value(&vars, &["HUB_ADDRESS"]),
            vss_verifier_address: first_value(&vars, &["VSS_VERIFIER_ADDRESS"]),
            vdd_verifier_address: first_value(&vars, &["VDD_VERIFIER_ADDRESS"]),
            subgraph_query_url: first_value(&vars, &["SUBGRAPH_QUERY_URL"]),
            oracle_worker_url: first_value(&vars, &["ORACLE_WORKER_URL"]),
            oracle_worker_token: first_value(&vars, &["ORACLE_WORKER_TOKEN"]),
            walrus_publisher_url: first_value(
                &vars,
                &["WALRUS_PUBLISHER_URL", "WALRUS_LOCAL_ENDPOINT"],
            ),
            walrus_aggregator_url: first_value(
                &vars,
                &["WALRUS_AGGREGATOR_URL", "WALRUS_LOCAL_ENDPOINT"],
            ),
            state_dir: first_value(&vars, &["DROP_CLI_STATE_DIR"]),
            owner_secret_key: parse_hex32(first_value(&vars, &["OWNER_SECRET_KEY"]).as_deref()),
            base_env_path: first_value(&vars, &["DROP_CLI_BASE_ENV"]),
        })
    }

    pub fn default_oracle_worker_url(&self) -> &str {
        self.oracle_worker_url
            .as_deref()
            .unwrap_or("https://trustdrop-oracle-worker.zhengxingao.workers.dev")
    }

    pub fn require_oracle_worker_token(&self) -> Result<&str> {
        self.oracle_worker_token
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("ORACLE_WORKER_TOKEN is missing"))
    }

    pub fn require_asset_encryption_key(&self) -> Result<[u8; 32]> {
        let seller_key = self
            .seller_private_key
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("SELLER_KEY is missing; cannot derive asset encryption key"))?;
        derive_asset_encryption_key_from_seller_key(seller_key, self.chain_id)
    }

    pub fn require_owner_secret_key(&self) -> Result<[u8; 32]> {
        if let Some(key) = self.owner_secret_key {
            return Ok(key);
        }
        Err(anyhow!(
            "OWNER_SECRET_KEY is missing; set an explicit 32-byte hex key"
        ))
    }

    pub fn require_sp1_private_key(&self) -> Result<&str> {
        self.sp1_private_key
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("SP1_PRIVATE_KEY is missing"))
    }
}

fn parse_env_with_base(path: impl AsRef<Path>) -> Result<BTreeMap<String, String>> {
    let mut vars = parse_env_file(&path)?;
    if let Some(base_path) = first_value(&vars, &["DROP_CLI_BASE_ENV"]) {
        let mut merged = parse_env_file(base_path)?;
        merged.extend(vars);
        vars = merged;
    }
    for key in [
        "DROP_CLI_STATE_DIR",
        "OWNER_SECRET_KEY",
        "SP1_PRIVATE_KEY",
        "NETWORK_PRIVATE_KEY",
    ] {
        if let Ok(value) = env::var(key) {
            vars.insert(key.to_string(), value);
        }
    }
    Ok(vars)
}

pub fn parse_env_file(path: impl AsRef<Path>) -> Result<BTreeMap<String, String>> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .map_err(|error| anyhow!("failed to read {}: {}", path.display(), error))?;
    let mut vars = BTreeMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        vars.insert(key.trim().to_string(), unquote_env_value(value.trim()));
    }
    Ok(vars)
}

fn first_value(vars: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| vars.get(*key))
        .filter(|value| !value.is_empty())
        .cloned()
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn parse_hex32(value: Option<&str>) -> Option<[u8; 32]> {
    let value = value?;
    let clean = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(clean).ok()?;
    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquote_env_values() {
        assert_eq!(unquote_env_value("\"abc\""), "abc");
        assert_eq!(unquote_env_value("'abc'"), "abc");
        assert_eq!(unquote_env_value("abc"), "abc");
    }
}
