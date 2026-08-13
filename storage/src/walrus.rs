// src/walrus.rs
use async_trait::async_trait;
use bytes::Bytes;
use futures::TryStreamExt;
use reqwest::StatusCode;
use reqwest::Client;
use serde::Deserialize;
use tokio_util::io::StreamReader;
use std::io;

use crate::{ BlobId, BlobStatus, WalrusConfig, StorageError, StorageNetwork, WalrusUploadResponse };

/// WalrusClient implements the StorageNetwork trait you provided.
///
/// Behavior:
/// - upload_blob -> PUT {publisher_url}/v1/blobs?epochs={epoch}[&send_object_to=...]
/// - get_status  -> GET {blockberry_base}/v1/blobs/{blobId} with header x-api-key
/// - download    -> GET {aggregator_url}/v1/blobs/{blobId} returning async reader
pub struct WalrusClient {
    pub cfg: WalrusConfig,
    http: Client,
}

impl WalrusClient {
    pub fn new(cfg: WalrusConfig) -> Self {
        let http: Client = Client::builder()
            //.pool_max_idle_per_host(0)
            //.http2_prior_knowledge()
            //.timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest client");
        Self {
            cfg,
            http,
        }
    }

    async fn configure(&mut self, cfg: WalrusConfig) {
        self.cfg = cfg;
    }

    pub async fn upload_blob_response(
        &self,
        data: Bytes,
        extra: Option<&str>,
    ) -> Result<WalrusUploadResponse, StorageError> {
        let epoch: u32 = extra.and_then(|s| s.parse::<u32>().ok()).unwrap_or(4);

        let mut url = format!(
            "{}/v1/blobs?epochs={}",
            self.cfg.publisher_url.trim_end_matches('/'),
            epoch
        );
        if let Some(addr) = &self.cfg.send_object_to {
            if !addr.is_empty() {
                url.push_str(&format!("&send_object_to={}", addr));
            }
        }

        let resp = self.http
            .put(&url)
            .header("Content-Type", "application/octet-stream")
            .header("Transfer-Encoding", "chunked")
            .body(data)
            .send().await
            .map_err(|e|
                StorageError::Other(
                    format!(
                        "Upload failed!\nURL: {}\nMethod: PUT\nError: {}\nSource: {:?}",
                        url,
                        e,
                        e
                    )
                )
            )?;

        if !resp.status().is_success() {
            let code = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(StorageError::Other(format!(
                "unexpected status code {code}; body: {body}"
            )));
        }

        let v = resp
            .json::<serde_json::Value>().await
            .map_err(|e| {
                StorageError::Other(format!("failed to parse upload response json: {}", e))
            })?;

        WalrusUploadResponse::from_json(v).map_err(|e|
            StorageError::Other(format!("upload response format error: {}", e))
        )
    }
}

#[derive(Debug, Deserialize)]
struct BlockberryMetadata {
    #[serde(rename = "blobId")]
    blob_id: Option<String>,
    #[serde(rename = "blobIdBase64")]
    blob_id_base64: Option<String>,
    #[serde(rename = "suiObjectId")]
    sui_object_id: Option<String>,
    #[serde(rename = "startEpoch")]
    start_epoch: Option<u64>,
    #[serde(rename = "endEpoch")]
    end_epoch: Option<u64>,
    size: Option<u64>,
    // other fields ignored
}

#[async_trait]
impl StorageNetwork for WalrusClient {
    /// Upload bytes. `extra` accepts optional string; for Walrus we expect
    /// extra may contain an epoch number as plain string (e.g. "8").
    /// Default epoch = 4.
    async fn upload_blob(&self, data: Bytes, extra: Option<&str>) -> Result<BlobId, StorageError> {
        Ok(self.upload_blob_response(data, extra).await?.blob_id())
    }

    /// get_status MUST call Blockberry metadata API (not aggregator).
    /// Endpoint: {blockberry_base}/v1/blobs/{blobId} with header x-api-key.
    /// - prints response body
    /// - returns BlobStatus::Info on 200
    /// - returns BlobStatus::NotFound on 404
    /// - returns BlobStatus::Error(...) on other non-200
    async fn get_status(&self, blob: &BlobId) -> Result<BlobStatus, StorageError> {
        let url = format!("{}/v1/blobs/{}", self.cfg.blockberry_base.trim_end_matches('/'), blob.0);

        let req = self.http
            .get(&url)
            .header("x-api-key", &self.cfg.api_key)
            .header("accept", "*/*");

        let resp = req
            .send().await
            .map_err(|e| { StorageError::Other(format!("request to blockberry failed: {}", e)) })?;

        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_else(|_| "<body parse failed>".into());

        // print for debug as you requested
        // println!("Blockberry status API [{}]: {}", status, body);

        match status {
            200 => {
                // parse
                let meta: BlockberryMetadata = serde_json
                    ::from_str(&body)
                    .map_err(|e| {
                        StorageError::Other(format!("failed to parse blockberry json: {}", e))
                    })?;

                // Extract fields with reasonable fallbacks
                let start_epoch = meta.start_epoch.unwrap_or(0);
                let end_epoch = meta.end_epoch.unwrap_or(0);
                let size = meta.size.unwrap_or(0);
                let sui_object_id = meta.sui_object_id.unwrap_or_default();
                let blob_id = meta.blob_id_base64
                    .or(meta.blob_id)
                    .unwrap_or_else(|| blob.0.clone());

                Ok(BlobStatus::Info {
                    blob_id,
                    start_epoch,
                    end_epoch,
                    size,
                })
            }
            404 => Ok(BlobStatus::NotFound),
            _ => Ok(BlobStatus::Error(format!("status {}: {}", status, body))),
        }
    }

    /// download returns an AsyncRead boxed. Uses aggregator to fetch raw blob bytes.
    /// Endpoint: {aggregator_url}/v1/blobs/{blobId}
    async fn download_blob(
        &self,
        blob: &BlobId
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, StorageError> {
        let url = format!("{}/v1/blobs/{}", self.cfg.aggregator_url.trim_end_matches('/'), blob.0);

        let resp: reqwest::Response = self.http
            .get(&url)
            .send().await
            .map_err(|e|
                StorageError::Other(
                    format!(
                        "Download failed!\nURL: {}\nMethod: GET\nError: {}\nSource: {:?}",
                        url,
                        e,
                        e
                    )
                )
            )?;

        match resp.status() {
            StatusCode::OK => {
                let byte_stream = resp
                    .bytes_stream()
                    .map_err(|e| {
                        io::Error::new(io::ErrorKind::Other, format!("reqwest stream error: {}", e))
                    });
                let reader = StreamReader::new(byte_stream);
                Ok(Box::new(reader))
            }
            StatusCode::NOT_FOUND =>
                Err(StorageError::Other(format!("Blob not found: {}", blob.0))),
            other => {
                let body = resp.text().await.unwrap_or_default();
                Err(
                    StorageError::Other(
                        format!("Download failed! Status: {}\nBody: {}", other, body)
                    )
                )
            }
        }
    }
}
