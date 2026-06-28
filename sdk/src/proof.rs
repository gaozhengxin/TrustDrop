use anyhow::Result;
use ethers::prelude::*;

/// Placeholder VSS proof helper.
///
/// This does not produce a production SP1 proof. The live Prove Network flow is
/// still owned by `drop-script` until the proof provider layer is migrated.
pub async fn run_vss_proof(v_k: [u8; 32], d_k: [u8; 32]) -> Result<(Bytes, Bytes)> {
    let _ = (v_k, d_k);
    Ok((vec![0u8; 64].into(), vec![0u8; 160].into()))
}

/// Placeholder VDD proof helper.
///
/// This does not produce a production SP1 proof. The live Prove Network flow is
/// still owned by `drop-script` until the proof provider layer is migrated.
pub async fn run_vdd_proof(o: [u8; 32], c: [u8; 32], k: [u8; 32]) -> Result<(Bytes, Bytes)> {
    let _ = (o, k);
    Ok((vec![0u8; 64].into(), c.to_vec().into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_vss_proof_returns_placeholder_shape() {
        let (proof, public_values) = run_vss_proof([1u8; 32], [2u8; 32]).await.unwrap();

        assert_eq!(proof.len(), 64);
        assert_eq!(public_values.len(), 160);
    }

    #[tokio::test]
    async fn run_vdd_proof_returns_placeholder_shape() {
        let c_cipher = [9u8; 32];
        let (proof, public_values) = run_vdd_proof([1u8; 32], c_cipher, [2u8; 32])
            .await
            .unwrap();

        assert_eq!(proof.len(), 64);
        assert_eq!(public_values.as_ref(), c_cipher.as_slice());
    }
}
