const MASTER_DOMAIN: &[u8] = b"TrustDrop.VideoSampling.v1";
const SPEC_DOMAIN: &[u8] = b"TrustDrop.VideoSampling.spec.v1";
const PROFILE: &[u8] = b"mp4-h264-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplingSeedInput {
    pub chain_id: u64,
    pub sale_contract: [u8; 20],
    pub sale_id: [u8; 32],
    pub origin_blob_id: [u8; 32],
    pub spec_hash: [u8; 32],
    pub external_randomness: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedError {
    InvalidPreviewDuration,
}

pub fn sampling_spec_hash(preview_duration_ms: u32) -> Result<[u8; 32], SeedError> {
    if preview_duration_ms == 0 {
        return Err(SeedError::InvalidPreviewDuration);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(SPEC_DOMAIN);
    hasher.update(&(PROFILE.len() as u32).to_be_bytes());
    hasher.update(PROFILE);
    hasher.update(&preview_duration_ms.to_be_bytes());
    Ok(*hasher.finalize().as_bytes())
}

pub fn derive_sampling_seed(input: &SamplingSeedInput) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MASTER_DOMAIN);
    hasher.update(&input.chain_id.to_be_bytes());
    hasher.update(&input.sale_contract);
    hasher.update(&input.sale_id);
    hasher.update(&input.origin_blob_id);
    hasher.update(&input.spec_hash);
    hasher.update(&input.external_randomness);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_context_bound_seed() {
        let spec_hash = sampling_spec_hash(5_000).unwrap();
        let input = SamplingSeedInput {
            chain_id: 421_614,
            sale_contract: [1; 20],
            sale_id: [2; 32],
            origin_blob_id: [3; 32],
            spec_hash,
            external_randomness: [4; 32],
        };
        let seed = derive_sampling_seed(&input);
        assert_eq!(seed, derive_sampling_seed(&input));
        let mut changed = input;
        changed.sale_id[31] ^= 1;
        assert_ne!(seed, derive_sampling_seed(&changed));
    }
}
