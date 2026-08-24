const CANDIDATE_DOMAIN: &[u8] = b"TrustDrop.VideoSampling.candidate.v1";

pub(super) fn bucket_boundary(
    start: i64,
    timeline_len: u64,
    bucket: u64,
    bucket_count: u64,
) -> Option<i64> {
    let offset = (timeline_len as u128).checked_mul(bucket as u128)? / bucket_count as u128;
    start.checked_add(i64::try_from(offset).ok()?)
}

pub(super) fn unbiased_offset(
    seed: &[u8; 32],
    bucket: u32,
    mut counter: u64,
    range: u64,
) -> u64 {
    let zone = (1u128 << 64) - ((1u128 << 64) % range as u128);
    loop {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CANDIDATE_DOMAIN);
        hasher.update(seed);
        hasher.update(&bucket.to_be_bytes());
        hasher.update(&counter.to_be_bytes());
        let digest = hasher.finalize();
        let value = u64::from_be_bytes(digest.as_bytes()[..8].try_into().expect("8 bytes"));
        if (value as u128) < zone {
            return value % range;
        }
        counter = counter.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partitions_without_losing_ticks() {
        assert_eq!(bucket_boundary(10, 10, 0, 3), Some(10));
        assert_eq!(bucket_boundary(10, 10, 1, 3), Some(13));
        assert_eq!(bucket_boundary(10, 10, 2, 3), Some(16));
        assert_eq!(bucket_boundary(10, 10, 3, 3), Some(20));
    }

    #[test]
    fn candidate_is_deterministic_and_in_range() {
        let first = unbiased_offset(&[7; 32], 1, 0, 17);
        assert_eq!(first, unbiased_offset(&[7; 32], 1, 0, 17));
        assert!(first < 17);
    }
}
