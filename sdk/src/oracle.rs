use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OracleWorkerClient {
    base_url: String,
    token: String,
    http: Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReadiness {
    pub ok: bool,
    #[serde(default)]
    pub chain_id: Option<u64>,
    #[serde(default)]
    pub oracle_proxy_configured: Option<bool>,
    #[serde(default)]
    pub relayer_configured: Option<bool>,
    #[serde(default)]
    pub relayer_matches_oracle_proxy: Option<bool>,
    #[serde(default)]
    pub relayer_balance_sufficient: Option<bool>,
    #[serde(default)]
    pub relayer_has_pending_tx: Option<bool>,
    #[serde(default)]
    pub walrus_api_configured: Option<bool>,
    #[serde(default)]
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobStatus {
    pub ok: bool,
    pub blob_id: String,
    pub found: bool,
    pub expired: bool,
    pub status: u8,
    pub status_name: String,
    pub end_epoch: Option<u64>,
    pub end_time: u64,
    pub expires_at: Option<String>,
    pub upstream_status: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleFulfillResult {
    pub ok: bool,
    #[serde(default)]
    pub already_fulfilled: bool,
    #[serde(default)]
    pub report_tx_hash: Option<String>,
}

impl OracleWorkerClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            http: Client::new(),
        }
    }

    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        let response = self.http.get(url).send().await?;
        Ok(response.status().is_success())
    }

    pub async fn status(&self) -> Result<WorkerReadiness> {
        self.get_json("/status").await
    }

    pub async fn blob_status_by_blob_id(&self, blob_id: &str) -> Result<BlobStatus> {
        self.get_json(&format!("/walrus/blob-status?blobId={}", blob_id))
            .await
    }

    pub async fn blob_status_by_c_cipher(&self, c_cipher: &str) -> Result<BlobStatus> {
        self.get_json(&format!("/walrus/blob-status?cCipher={}", c_cipher))
            .await
    }

    pub async fn fulfill(
        &self,
        chain_id: u64,
        tx_hash: &str,
        walrus_end_epoch: Option<u64>,
    ) -> Result<OracleFulfillResult> {
        let url = format!("{}/oracle/fulfill", self.base_url);
        let mut body = serde_json::json!({
            "chainId": chain_id,
            "txHash": tx_hash,
        });
        if let Some(end_epoch) = walrus_end_epoch {
            body["walrusEndEpoch"] = serde_json::json!(end_epoch);
        }
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("oracle worker returned {}: {}", status, body));
        }
        Ok(response.json::<OracleFulfillResult>().await?)
    }

    async fn get_json<T>(&self, path_and_query: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.base_url, path_and_query);
        let response = self.http.get(url).bearer_auth(&self.token).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("oracle worker returned {}: {}", status, body));
        }
        Ok(response.json::<T>().await?)
    }
}
