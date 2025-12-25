use serde::{ Deserialize, Serialize };

// --- Walrus 配置 ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalrusConfig {
    pub publisher_url: String,
    pub aggregator_url: String,
    pub blockberry_base: String,
    pub api_key: String,
    pub send_object_to: Option<String>,
}

// --- Filecoin (Lighthouse) 配置 ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilecoinConfig {
    pub ipfs_rpc_url: String,
    pub gateway_url: String,
    pub lighthouse_api_key: String,
}

impl Default for FilecoinConfig {
    fn default() -> Self {
        Self {
            ipfs_rpc_url: "http://127.0.0.1:5001".into(),
            gateway_url: "https://gateway.lighthouse.storage/ipfs/".into(),
            lighthouse_api_key: "".into(),
        }
    }
}

impl FilecoinConfig {
    pub fn new(lighthouse_api_key: impl Into<String>) -> Self {
        Self {
            lighthouse_api_key: lighthouse_api_key.into(),
            ..Default::default()
        }
    }
}
