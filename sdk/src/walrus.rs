use anyhow::{Result, anyhow};
use drop_lib::walrus_address::compute_blob_id_default;
use storage::WalrusClient;
use tokio::time::{Duration, sleep, timeout};

pub fn compute_rs_id(data: &[u8]) -> Result<[u8; 32]> {
    let id_raw = compute_blob_id_default(data).map_err(|e| anyhow!(e))?;
    Ok((*id_raw.as_ref())
        .try_into()
        .map_err(|_| anyhow!("ID Length Error"))?)
}

pub async fn upload_data_idempotent(walrus: &WalrusClient, data: Vec<u8>) -> Result<String> {
    Ok(upload_data_idempotent_with_end_epoch(walrus, data).await?.0)
}

pub async fn upload_data_idempotent_with_end_epoch(
    walrus: &WalrusClient,
    data: Vec<u8>,
) -> Result<(String, Option<u64>)> {
    let mut last_error = None::<String>;
    for attempt in 1..=3 {
        match timeout(
            Duration::from_secs(120),
            walrus.upload_blob_response(data.clone().into(), Some("4")),
        )
        .await
        {
            Ok(Ok(uploaded)) => return Ok((uploaded.blob_id().0, Some(uploaded.end_epoch()))),
            Ok(Err(error)) => {
                last_error = Some(error.to_string());
                eprintln!("walrus upload attempt {attempt}/3 failed: {error}");
            }
            Err(_) => {
                let error = "walrus upload attempt timed out after 120 seconds".to_string();
                eprintln!("walrus upload attempt {attempt}/3 failed: {error}");
                last_error = Some(error);
            }
        }

        if attempt < 3 {
            sleep(Duration::from_secs(10 * attempt)).await;
        }
    }

    Err(anyhow!(
        "walrus upload failed after retries: {}",
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
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
