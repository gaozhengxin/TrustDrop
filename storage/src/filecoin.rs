use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use reqwest::{ multipart, Client };
use serde_json::Value;
use tokio::io::AsyncRead;
use crate::{ BlobId, BlobStatus, FilecoinConfig, StorageError, StorageNetwork };

pub struct FilecoinClient {
    pub cfg: FilecoinConfig,
    http: Client,
}

impl FilecoinClient {
    pub fn new(cfg: FilecoinConfig) -> Self {
        Self {
            cfg: cfg,
            http: reqwest::Client::new(),
        }
    }

    async fn configure(&mut self, cfg: FilecoinConfig) {
        self.cfg = cfg;
    }
}

#[async_trait]
impl StorageNetwork for FilecoinClient {
    // 功能 1 & 2: 上传并下单 (Lighthouse 会自动处理私钥关联的账户资金)
    async fn upload_blob(&self, data: Bytes, _extra: Option<&str>) -> Result<BlobId, StorageError> {
        let api_key = &self.cfg.api_key;

        let part = multipart::Part
            ::bytes(data.to_vec())
            .file_name("file.dat")
            .mime_str("application/octet-stream")
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let form = multipart::Form::new().part("file", part);

        let res = self.http
            .post(&self.cfg.url)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send().await
            .map_err(StorageError::Http)?;

        if !res.status().is_success() {
            let status_code = res.status();
            let txt = res.text().await.unwrap_or_default();
            return Err(StorageError::Other(format!("Server error {}: {}", status_code, txt)));
        }

        let json: serde_json::Value = res
            .json().await
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let cid = json["Hash"].as_str().ok_or_else(|| StorageError::Other("No Hash field".into()))?;

        Ok(BlobId(cid.to_string()))
    }

    // 功能 4: 检索 Deal 状态
    async fn get_status(&self, blob: &BlobId) -> Result<BlobStatus, StorageError> {
        let url = format!(
            //"https://api.lighthouse.storage/api/lighthouse/get_indexing_info?cid={}",
            //"https://api.lighthouse.storage/api/get_indexing_info?cid={}",
            "https://api.lighthouse.storage/api/lighthouse/deal_status?cid={}",
            blob.0
        );

        let res = self.http
            .get(url)
            .send().await
            .map_err(|e| StorageError::Http(e))?;

        let text = res.text().await.map_err(|e| StorageError::Other(e.to_string()))?;
        let json: Value = serde_json
            ::from_str(&text)
            .map_err(|e| {
                StorageError::Other(format!("JSON parse error: {}, body: {}", e, text))
            })?;
        //let json: Value = res.json().await.map_err(|e| StorageError::Other(e.to_string()))?;
        let info = &json["deal_info"][0];

        let parse_u64 = |v: &serde_json::Value| -> u64 {
            if let Some(n) = v.as_u64() {
                n
            } else if let Some(s) = v.as_str() {
                s.parse::<u64>().unwrap_or(0)
            } else {
                0
            }
        };

        Ok(BlobStatus::InfoFC {
            cid: blob.0.clone(),
            deal_id: parse_u64(&info["dealId"]),
            start_epoch: parse_u64(&info["storage_start_epoch"]),
            end_epoch: parse_u64(&info["storage_end_epoch"]),
            status: "Active".to_string(),
        })
    }

    // 功能 3: 任何人可直接通过网关下载
    async fn download_blob(
        &self,
        blob: &BlobId
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
        let url = format!("{}{}", self.cfg.gateway_url, blob.0);

        let res = self.http
            .get(url)
            .send().await
            .map_err(|e| StorageError::Http(e))?;

        if !res.status().is_success() {
            return Err(
                StorageError::Other(format!("Download failed with status: {}", res.status()))
            );
        }

        // 将 reqwest 的 stream 转换为 AsyncRead
        let stream = res.bytes_stream();
        let reader = tokio_util::io::StreamReader::new(
            stream.map(|item| item.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
        );

        Ok(Box::new(reader))
    }
}
