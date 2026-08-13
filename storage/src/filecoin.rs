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
        let lighthouse_api_key = &self.cfg.lighthouse_api_key;

        let ipfs_add_url = format!(
            "{}/api/v0/add?cid-version=1&raw-leaves=true",
            self.cfg.ipfs_rpc_url
        );

        let part = multipart::Part
            ::bytes(data.to_vec())
            .file_name("file.dat")
            .mime_str("application/octet-stream")
            .map_err(|e| StorageError::Other(e.to_string()))?;

        let form = multipart::Form::new().part("file", part);

        let ipfs_res = self.http
            .post(&ipfs_add_url)
            .multipart(form)
            .send().await
            .map_err(StorageError::Http)?;

        if !ipfs_res.status().is_success() {
            let status = ipfs_res.status();
            let err_text = ipfs_res.text().await.unwrap_or_default();
            return Err(
                StorageError::Other(format!("IPFS local node error {}: {}", status, err_text))
            );
        }

        let ipfs_json: serde_json::Value = ipfs_res
            .json().await
            .map_err(|e| StorageError::Other(format!("Failed to parse IPFS response: {}", e)))?;

        let cid = ipfs_json["Hash"]
            .as_str()
            .ok_or_else(|| StorageError::Other("IPFS response missing 'Hash' field".into()))?;

        // --- 逻辑改动 1: ipfs add 成功后，先等待 3 秒，让 P2P 网络同步 ---
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let body_json =
            serde_json::json!({
            "cid": cid,
            "fileName": "file.dat"
        });

        let lighthouse_url = "https://api.lighthouse.storage";
        let pin_url = format!("{}/api/lighthouse/pin", lighthouse_url.trim_end_matches('/'));

        // --- 逻辑改动 2: 重试循环 ---
        let mut attempts = 0;
        let max_attempts = 5;
        let mut last_err = String::new();

        while attempts < max_attempts {
            attempts += 1;

            let lighthouse_res = self.http
                .post(&pin_url)
                .header("Authorization", format!("Bearer {}", lighthouse_api_key))
                .header("Content-Type", "application/json")
                .json(&body_json) // 直接传 json 对象，reqwest 会自动处理序列化
                .send().await
                .map_err(StorageError::Http)?;

            let status = lighthouse_res.status();
            let res_text = lighthouse_res.text().await.unwrap_or_default();

            // 打印 response 用于调试
            println!(
                "DEBUG: Lighthouse pin attempt {}/{} for CID: {}",
                attempts,
                max_attempts,
                cid
            );
            println!("DEBUG: Response Status: {}, Body: {}", status, res_text);

            if status.is_success() {
                return Ok(BlobId(cid.to_string()));
            } else {
                last_err = res_text;

                if attempts < max_attempts {
                    let wait_secs = attempts * 3; // 逐步增加等待时间: 3, 6, 9...
                    println!("DEBUG: Pin failed, retrying in {} seconds...", wait_secs);
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }
            }
        }

        Err(
            StorageError::Other(
                format!(
                    "Lighthouse pin failed after {} attempts. Last response: {}",
                    max_attempts,
                    last_err
                )
            )
        )
    }

    // 功能 4: 检索 file 状态
    async fn get_status(&self, blob: &BlobId) -> Result<BlobStatus, StorageError> {
        // 1. 先查 file_info 确定是否 Pin 成功
        let file_info_url = format!(
            "https://api.lighthouse.storage/api/lighthouse/file_info?cid={}",
            blob.0
        );

        let file_info_res = self.http.get(file_info_url).send().await?;

        // 如果 file_info 返回 404，直接判定为 NotFound (未 Pin)
        if file_info_res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(BlobStatus::NotFound);
        }

        if !file_info_res.status().is_success() {
            return Ok(
                BlobStatus::Error(format!("file_info API error: {}", file_info_res.status()))
            );
        }

        // 2. 如果 file_info 成功，再查 deal_status 获取详细上链状态
        let deal_status_url = format!(
            "https://api.lighthouse.storage/api/lighthouse/deal_status?cid={}",
            blob.0
        );

        let deal_res = self.http.get(deal_status_url).send().await?;
        let text = deal_res.text().await.map_err(|e| StorageError::Other(e.to_string()))?;

        let json: Value = serde_json
            ::from_str(&text)
            .map_err(|e| {
                StorageError::Other(format!("JSON parse error: {}, body: {}", e, text))
            })?;

        // 提取 deal 信息 (注意处理数组可能为空的情况)
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

        let deal_id = parse_u64(&info["dealId"]);

        // 定义内部状态逻辑
        // 如果 dealId 为 0，状态设为 "Pinned" (已 Pin 到热存储，但未同步到 Filecoin 扇区)
        // 如果 dealId > 0，状态设为 "Active" (已成功创建上链 Deal)
        let status_str = if deal_id == 0 { "Pinned".to_string() } else { "Active".to_string() };

        Ok(BlobStatus::InfoFC {
            cid: blob.0.clone(),
            deal_id,
            start_epoch: parse_u64(&info["storage_start_epoch"]),
            end_epoch: parse_u64(&info["storage_end_epoch"]),
            status: status_str,
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
