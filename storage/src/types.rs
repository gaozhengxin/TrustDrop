use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobId(pub String);

#[derive(Debug, Clone)]
pub enum BlobStatus {
    /// Blob 存在，并包含若干元信息
    Info {
        blob_id: String,
        start_epoch: u64,
        end_epoch: u64,
        size: u64,
    },
    /// Blob 在 Walrus 中不存在（404）
    NotFound,
    /// 其它错误（解析失败、网络异常等），携带人类可读的信息
    Error(String),
}

#[derive(Debug)]
pub struct NewlyCreatedInfo {
    pub blob_id: BlobId,
    pub end_epoch: u64,
    pub sui_object_id: String,
}

#[derive(Debug)]
pub struct AlreadyCertifiedInfo {
    pub blob_id: BlobId,
    pub end_epoch: u64,
    pub tx_digest: String,
}

#[derive(Debug)]
pub enum WalrusUploadResponse {
    NewlyCreated(NewlyCreatedInfo),
    AlreadyCertified(AlreadyCertifiedInfo),
}

impl WalrusUploadResponse {
    pub fn from_json(v: Value) -> Result<Self, crate::StorageError> {
        if let Some(obj) = v.get("newlyCreated") {
            let blob_obj = obj.get("blobObject").unwrap();
            Ok(Self::NewlyCreated(NewlyCreatedInfo {
                blob_id: BlobId(blob_obj.get("blobId").unwrap().as_str().unwrap().into()),
                end_epoch: blob_obj
                    .get("storage")
                    .unwrap()
                    .get("endEpoch")
                    .unwrap()
                    .as_u64()
                    .unwrap(),
                sui_object_id: blob_obj.get("id").unwrap().as_str().unwrap().into(),
            }))
        } else if let Some(obj) = v.get("alreadyCertified") {
            Ok(Self::AlreadyCertified(AlreadyCertifiedInfo {
                blob_id: BlobId(obj.get("blobId").unwrap().as_str().unwrap().into()),
                end_epoch: obj.get("endEpoch").unwrap().as_u64().unwrap(),
                tx_digest: obj
                    .get("event")
                    .unwrap()
                    .get("txDigest")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .into(),
            }))
        } else {
            Err(crate::StorageError::Other("unknown walrus response".into()))
        }
    }

    pub fn blob_id(&self) -> BlobId {
        match self {
            Self::NewlyCreated(n) => n.blob_id.clone(),
            Self::AlreadyCertified(a) => a.blob_id.clone(),
        }
    }
}
