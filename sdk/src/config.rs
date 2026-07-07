use anyhow::{Result, anyhow};
use std::{collections::BTreeMap, env, fs, path::Path};

#[derive(Debug, Clone, Default)]
pub struct DropCliConfig {
    pub rpc_url: Option<String>,
    pub chain_id: u64,
    pub seller_private_key: Option<String>,
    pub buyer_private_key: Option<String>,
    pub hub_address: Option<String>,
    pub subgraph_query_url: Option<String>,
    pub oracle_worker_url: Option<String>,
    pub oracle_worker_token: Option<String>,
    pub walrus_publisher_url: Option<String>,
    pub walrus_aggregator_url: Option<String>,
    pub state_dir: Option<String>,
    pub asset_encryption_key: Option<[u8; 32]>,
    pub owner_secret_key: Option<[u8; 32]>,
    pub dev_insecure_default_keys: bool,
    pub base_env_path: Option<String>,
}

impl DropCliConfig {
    pub fn from_env_file(path: impl AsRef<Path>) -> Result<Self> {
        let vars = parse_env_with_base(path)?;
        let dev_insecure_default_keys =
            first_value(&vars, &["TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS"])
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
        Ok(Self {
            rpc_url: first_value(&vars, &["ARBITRUM_SEPOLIA_RPC_URL", "ARBITRUM_SEPOLIA_RPC"]),
            chain_id: first_value(&vars, &["CHAIN_ID"])
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(421614),
            seller_private_key: first_value(&vars, &["SELLER_KEY", "PRIVATE_KEY"]),
            buyer_private_key: first_value(&vars, &["BUYER_KEY"]),
            hub_address: first_value(&vars, &["HUB_ADDRESS"]),
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
            asset_encryption_key: parse_hex32(
                first_value(&vars, &["ASSET_ENCRYPTION_KEY"]).as_deref(),
            ),
            owner_secret_key: parse_hex32(first_value(&vars, &["OWNER_SECRET_KEY"]).as_deref()),
            dev_insecure_default_keys,
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
        if let Some(key) = self.asset_encryption_key {
            return Ok(key);
        }
        if self.dev_insecure_default_keys {
            return Ok([0x22; 32]);
        }
        Err(anyhow!(
            "ASSET_ENCRYPTION_KEY is missing; set it explicitly or set TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS=1 for prototype testing"
        ))
    }

    pub fn require_owner_secret_key(&self) -> Result<[u8; 32]> {
        if let Some(key) = self.owner_secret_key {
            return Ok(key);
        }
        if self.dev_insecure_default_keys {
            return Ok([0x11; 32]);
        }
        Err(anyhow!(
            "OWNER_SECRET_KEY is missing; set it explicitly or set TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS=1 for prototype testing"
        ))
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
        "TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS",
        "ASSET_ENCRYPTION_KEY",
        "OWNER_SECRET_KEY",
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
