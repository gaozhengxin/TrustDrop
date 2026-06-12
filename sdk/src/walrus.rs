use anyhow::{Result, anyhow};
use drop_lib::walrus_address::compute_blob_id_default;
use storage::{WalrusClient, BlobId, BlobStatus, StorageNetwork};

pub fn compute_rs_id(data: &[u8]) -> Result<[u8; 32]> {
    let id_raw = compute_blob_id_default(data).map_err(|e| anyhow!(e))?;
    Ok((*id_raw.as_ref()).try_into().map_err(|_| anyhow!("ID Length Error"))?)
}

pub async fn upload_data_idempotent(walrus: &WalrusClient, data: Vec<u8>) -> Result<String> {
    let target_id = compute_rs_id(&data)?;
    let blob_id_hex = hex::encode(target_id);
    if let Ok(BlobStatus::Info { .. }) = walrus.get_status(&BlobId(blob_id_hex.clone())).await {
        return Ok(blob_id_hex);
    }
    let uploaded = walrus.upload_blob(data.into(), Some("4")).await?;
    Ok(uploaded.0)
}