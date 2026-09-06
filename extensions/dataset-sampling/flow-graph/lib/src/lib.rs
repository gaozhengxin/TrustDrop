#![no_std]

const WINDOW_DOMAIN: &[u8] = b"TrustDrop.FlowGraphSampling.window.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeCoverage {
    /// Timestamp of the first transfer represented by the dataset.
    pub first_timestamp: u64,
    /// Timestamp of the last transfer represented by the dataset, inclusive.
    pub last_timestamp: u64,
    /// Width of each canonical flow bucket in seconds.
    pub bucket_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeWindow {
    /// Inclusive, bucket-aligned start timestamp.
    pub bucket_start: u64,
    /// Exclusive, bucket-aligned end timestamp.
    pub bucket_end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingError {
    EmptyCoverage,
    InvalidBucketWidth,
    ArithmeticOverflow,
}

/// Selects one canonical bucket intersecting the dataset's declared coverage.
///
/// The first and last buckets may be partial because the manifest records the
/// first and last observed transfers rather than independent coverage bounds.
pub fn derive_time_window(
    seed: &[u8; 32],
    coverage: TimeCoverage,
) -> Result<TimeWindow, SamplingError> {
    if coverage.bucket_seconds == 0 {
        return Err(SamplingError::InvalidBucketWidth);
    }
    if coverage.first_timestamp > coverage.last_timestamp {
        return Err(SamplingError::EmptyCoverage);
    }

    let first_bucket = align_down(coverage.first_timestamp, coverage.bucket_seconds);
    let last_bucket = align_down(coverage.last_timestamp, coverage.bucket_seconds);
    let distance = last_bucket
        .checked_sub(first_bucket)
        .ok_or(SamplingError::ArithmeticOverflow)?;
    let bucket_count = distance
        .checked_div(coverage.bucket_seconds)
        .and_then(|value| value.checked_add(1))
        .ok_or(SamplingError::ArithmeticOverflow)?;
    let bucket_offset = unbiased_index(seed, bucket_count);
    let bucket_start = bucket_offset
        .checked_mul(coverage.bucket_seconds)
        .and_then(|value| first_bucket.checked_add(value))
        .ok_or(SamplingError::ArithmeticOverflow)?;
    let bucket_end = bucket_start
        .checked_add(coverage.bucket_seconds)
        .ok_or(SamplingError::ArithmeticOverflow)?;

    Ok(TimeWindow {
        bucket_start,
        bucket_end,
    })
}

/// Checks that a certificate records exactly the window derived from its seed.
pub fn verify_time_window(
    seed: &[u8; 32],
    coverage: TimeCoverage,
    claimed: TimeWindow,
) -> Result<bool, SamplingError> {
    Ok(derive_time_window(seed, coverage)? == claimed)
}

/// Checks whether an aggregated flow row belongs to the selected bucket.
pub fn row_is_in_window(window: TimeWindow, row_bucket_start: u64, row_bucket_end: u64) -> bool {
    row_bucket_start == window.bucket_start && row_bucket_end == window.bucket_end
}

fn align_down(timestamp: u64, bucket_seconds: u64) -> u64 {
    timestamp - timestamp % bucket_seconds
}

fn unbiased_index(seed: &[u8; 32], upper_bound: u64) -> u64 {
    let zone = (1u128 << 64) - ((1u128 << 64) % u128::from(upper_bound));
    let mut counter = 0u64;
    loop {
        let mut hasher = blake3::Hasher::new();
        hasher.update(WINDOW_DOMAIN);
        hasher.update(seed);
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

    const COVERAGE: TimeCoverage = TimeCoverage {
        first_timestamp: 1_787_875_201,
        last_timestamp: 1_787_877_341,
        bucket_seconds: 60,
    };

    #[test]
    fn selection_is_deterministic_aligned_and_in_coverage() {
        let selected = derive_time_window(&[7; 32], COVERAGE).unwrap();
        assert_eq!(selected, derive_time_window(&[7; 32], COVERAGE).unwrap());
        assert_eq!(selected.bucket_start % COVERAGE.bucket_seconds, 0);
        assert_eq!(selected.bucket_end - selected.bucket_start, 60);
        assert!(selected.bucket_start <= COVERAGE.last_timestamp);
        assert!(selected.bucket_end > COVERAGE.first_timestamp);
    }

    #[test]
    fn a_single_bucket_has_only_one_possible_result() {
        let coverage = TimeCoverage {
            first_timestamp: 121,
            last_timestamp: 179,
            bucket_seconds: 60,
        };
        assert_eq!(
            derive_time_window(&[1; 32], coverage).unwrap(),
            TimeWindow {
                bucket_start: 120,
                bucket_end: 180,
            }
        );
    }

    #[test]
    fn verifies_claimed_window_and_row_membership() {
        let window = derive_time_window(&[9; 32], COVERAGE).unwrap();
        assert!(verify_time_window(&[9; 32], COVERAGE, window).unwrap());
        assert!(row_is_in_window(
            window,
            window.bucket_start,
            window.bucket_end
        ));
        assert!(!row_is_in_window(
            window,
            window.bucket_start + 60,
            window.bucket_end + 60
        ));
    }

    #[test]
    fn rejects_invalid_coverage() {
        assert_eq!(
            derive_time_window(
                &[0; 32],
                TimeCoverage {
                    first_timestamp: 2,
                    last_timestamp: 1,
                    bucket_seconds: 60,
                }
            ),
            Err(SamplingError::EmptyCoverage)
        );
        assert_eq!(
            derive_time_window(
                &[0; 32],
                TimeCoverage {
                    first_timestamp: 1,
                    last_timestamp: 2,
                    bucket_seconds: 0,
                }
            ),
            Err(SamplingError::InvalidBucketWidth)
        );
    }
}
