use anyhow::{Result, anyhow};
use drop_lib::walrus_address::compute_blob_id_default;
use storage::{BlobId, BlobStatus, StorageNetwork, WalrusClient};

pub fn compute_rs_id(data: &[u8]) -> Result<[u8; 32]> {
    let id_raw = compute_blob_id_default(data).map_err(|e| anyhow!(e))?;
    Ok((*id_raw.as_ref())
        .try_into()
        .map_err(|_| anyhow!("ID Length Error"))?)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_rs_id_is_deterministic() {
        let payload = b"trustdrop walrus payload";

        let a = compute_rs_id(payload).unwrap();
        let b = compute_rs_id(payload).unwrap();

        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn compute_rs_id_changes_with_payload() {
        let a = compute_rs_id(b"trustdrop walrus payload a").unwrap();
        let b = compute_rs_id(b"trustdrop walrus payload b").unwrap();

        assert_ne!(a, b);
    }
}
