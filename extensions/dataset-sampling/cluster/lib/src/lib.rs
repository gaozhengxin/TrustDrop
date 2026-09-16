#![no_std]

const INDEX_DOMAIN: &[u8] = b"TrustDrop.ClusterSampling.index.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingError {
    EmptyCatalog,
    EmptySelection,
    SelectionExceedsCatalog,
    ScratchLengthMismatch,
}

/// Selects unique zero-based records from the canonical wallet-cluster section.
///
/// `selected` is caller-owned so the guest does not need an allocator. The
/// resulting indices are sorted; the cluster IDs stored at those positions are
/// the IDs disclosed in the sample certificate.
pub fn derive_cluster_indices(
    seed: &[u8; 32],
    cluster_count: u64,
    selected: &mut [u64],
) -> Result<(), SamplingError> {
    if cluster_count == 0 {
        return Err(SamplingError::EmptyCatalog);
    }
    if selected.is_empty() {
        return Err(SamplingError::EmptySelection);
    }
    if u64::try_from(selected.len()).map_or(true, |count| count > cluster_count) {
        return Err(SamplingError::SelectionExceedsCatalog);
    }

    for ordinal in 0..selected.len() {
        let mut counter = 0u64;
        loop {
            let candidate = unbiased_index(seed, ordinal as u64, counter, cluster_count);
            if !selected[..ordinal].contains(&candidate) {
                selected[ordinal] = candidate;
                break;
            }
            counter = counter.wrapping_add(1);
        }
    }
    selected.sort_unstable();
    Ok(())
}

/// Checks that claimed canonical indices are exactly those derived from seed.
pub fn verify_cluster_indices(
    seed: &[u8; 32],
    cluster_count: u64,
    claimed: &[u64],
    scratch: &mut [u64],
) -> Result<bool, SamplingError> {
    if claimed.is_empty() {
        return Err(SamplingError::EmptySelection);
    }
    if scratch.len() != claimed.len() {
        return Err(SamplingError::ScratchLengthMismatch);
    }
    derive_cluster_indices(seed, cluster_count, scratch)?;
    Ok(scratch == claimed)
}

fn unbiased_index(seed: &[u8; 32], ordinal: u64, mut counter: u64, upper_bound: u64) -> u64 {
    let zone = (1u128 << 64) - ((1u128 << 64) % u128::from(upper_bound));
    loop {
        let mut hasher = blake3::Hasher::new();
        hasher.update(INDEX_DOMAIN);
        hasher.update(seed);
        hasher.update(&ordinal.to_be_bytes());
        hasher.update(&counter.to_be_bytes());
        let digest = hasher.finalize();
        let value = u64::from_be_bytes(digest.as_bytes()[..8].try_into().expect("8 bytes"));
        if u128::from(value) < zone {
            return value % upper_bound;
        }
        counter = counter.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_deterministic_unique_sorted_and_in_range() {
        let mut first = [0; 3];
        let mut second = [0; 3];
        derive_cluster_indices(&[7; 32], 55, &mut first).unwrap();
        derive_cluster_indices(&[7; 32], 55, &mut second).unwrap();
        assert_eq!(first, second);
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(first.iter().all(|index| *index < 55));
    }

    #[test]
    fn can_select_the_entire_catalog() {
        let mut selected = [0; 4];
        derive_cluster_indices(&[3; 32], 4, &mut selected).unwrap();
        assert_eq!(selected, [0, 1, 2, 3]);
    }

    #[test]
    fn verifies_exact_claimed_indices() {
        let mut claimed = [0; 3];
        derive_cluster_indices(&[11; 32], 100, &mut claimed).unwrap();
        assert!(verify_cluster_indices(&[11; 32], 100, &claimed, &mut [0; 3]).unwrap());
        claimed[0] = 100;
        assert!(!verify_cluster_indices(&[11; 32], 100, &claimed, &mut [0; 3]).unwrap());
    }

    #[test]
    fn rejects_impossible_requests() {
        assert_eq!(
            derive_cluster_indices(&[0; 32], 0, &mut [0; 1]),
            Err(SamplingError::EmptyCatalog)
        );
        assert_eq!(
            derive_cluster_indices(&[0; 32], 2, &mut []),
            Err(SamplingError::EmptySelection)
        );
        assert_eq!(
            derive_cluster_indices(&[0; 32], 2, &mut [0; 3]),
            Err(SamplingError::SelectionExceedsCatalog)
        );
        assert_eq!(
            verify_cluster_indices(&[0; 32], 2, &[0], &mut [0; 2]),
            Err(SamplingError::ScratchLengthMismatch)
        );
    }
}
