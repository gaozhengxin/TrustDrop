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
    pub url: String,
    pub api_key: String,
    pub gateway_url: String,
}

impl Default for FilecoinConfig {
    fn default() -> Self {
        Self {
            url: "https://upload.lighthouse.storage/api/v0/add?cid-version=1".into(),
            api_key: "".into(),
            gateway_url: "https://gateway.lighthouse.storage/ipfs/".into(),
        }
    }
}

impl FilecoinConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }
}
