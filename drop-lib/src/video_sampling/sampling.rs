extern crate alloc;

use alloc::vec::Vec;

use super::selection::{bucket_boundary, unbiased_offset};
use super::VideoTrack;

const BUCKET_COUNT: u64 = 3;
const MAX_ATTEMPTS_PER_BUCKET: u64 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplePlan {
    pub bucket_index: u8,
    pub target_time: i64,
    pub decode_start_sample: u32,
    pub decode_start_time: i64,
    pub presentation_end_time: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingError {
    InvalidTimeline,
    InvalidPreviewDuration,
    BucketTooShort(u8),
    NoSyncSample(u8),
    DuplicateAlignedSample(u8),
    IntegerOverflow,
}

pub fn plan_three_samples(
    track: &VideoTrack,
    master_seed: &[u8; 32],
    preview_duration_ms: u32,
) -> Result<[SamplePlan; 3], SamplingError> {
    if track.presentation_end <= track.presentation_start || track.timescale == 0 {
        return Err(SamplingError::InvalidTimeline);
    }
    if preview_duration_ms == 0 {
        return Err(SamplingError::InvalidPreviewDuration);
    }

    let preview_ticks = (preview_duration_ms as u64)
        .checked_mul(track.timescale as u64)
        .and_then(|value| value.checked_add(999))
        .ok_or(SamplingError::IntegerOverflow)?
        / 1_000;
    if preview_ticks == 0 || preview_ticks > i64::MAX as u64 {
        return Err(SamplingError::InvalidPreviewDuration);
    }

    let timeline_len = track
        .presentation_end
        .checked_sub(track.presentation_start)
        .ok_or(SamplingError::IntegerOverflow)? as u64;
    let mut plans = Vec::with_capacity(BUCKET_COUNT as usize);
    for bucket in 0..BUCKET_COUNT {
        let bucket_start = bucket_boundary(track.presentation_start, timeline_len, bucket, BUCKET_COUNT)
            .ok_or(SamplingError::IntegerOverflow)?;
        let bucket_end = bucket_boundary(
            track.presentation_start,
            timeline_len,
            bucket + 1,
            BUCKET_COUNT,
        )
        .ok_or(SamplingError::IntegerOverflow)?;
        let latest_target = bucket_end
            .checked_sub(preview_ticks as i64)
            .ok_or(SamplingError::BucketTooShort(bucket as u8))?;
        if latest_target < bucket_start {
            return Err(SamplingError::BucketTooShort(bucket as u8));
        }
        let candidate_count = latest_target
            .checked_sub(bucket_start)
            .and_then(|value| value.checked_add(1))
            .ok_or(SamplingError::IntegerOverflow)? as u64;

        let mut saw_sync = false;
        let mut selected = None;
        for counter in 0..MAX_ATTEMPTS_PER_BUCKET {
            let offset = unbiased_offset(master_seed, bucket as u32, counter, candidate_count);
            let target_time = bucket_start
                .checked_add(offset as i64)
                .ok_or(SamplingError::IntegerOverflow)?;
            let Some(sync) = track
                .samples
                .iter()
                .filter(|sample| sample.is_sync && sample.presentation_time <= target_time)
                .max_by_key(|sample| sample.presentation_time)
            else {
                continue;
            };
            saw_sync = true;
            if plans
                .iter()
                .any(|plan: &SamplePlan| plan.decode_start_sample == sync.index)
            {
                continue;
            }
            selected = Some(SamplePlan {
                bucket_index: bucket as u8,
                target_time,
                decode_start_sample: sync.index,
                decode_start_time: sync.presentation_time,
                presentation_end_time: target_time
                    .checked_add(preview_ticks as i64)
                    .ok_or(SamplingError::IntegerOverflow)?,
            });
            break;
        }
        plans.push(selected.ok_or(if saw_sync {
            SamplingError::DuplicateAlignedSample(bucket as u8)
        } else {
            SamplingError::NoSyncSample(bucket as u8)
        })?);
    }
    plans.try_into().map_err(|_| SamplingError::InvalidTimeline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_sampling::VideoSample;

    fn track() -> VideoTrack {
        let samples = (0..30)
            .map(|index| VideoSample {
                index,
                decode_time: index as u64 * 1_000,
                presentation_time: index as i64 * 1_000,
                duration: 1_000,
                byte_offset: index as u64 * 10,
                byte_size: 10,
                is_sync: index % 3 == 0,
            })
            .collect();
        VideoTrack {
            timescale: 1_000,
            media_duration: 30_000,
            presentation_start: 0,
            presentation_end: 30_000,
            codec: *b"avc1",
            samples,
        }
    }

    #[test]
    fn samples_one_complete_preview_per_time_bucket() {
        let plans = plan_three_samples(&track(), &[9; 32], 5_000).unwrap();
        for (index, plan) in plans.iter().enumerate() {
            let start = index as i64 * 10_000;
            let end = (index as i64 + 1) * 10_000;
            assert_eq!(plan.bucket_index, index as u8);
            assert!(plan.target_time >= start);
            assert!(plan.presentation_end_time <= end);
            assert!(plan.decode_start_time <= plan.target_time);
        }
        assert_ne!(plans[0].decode_start_sample, plans[1].decode_start_sample);
        assert_ne!(plans[1].decode_start_sample, plans[2].decode_start_sample);
    }

    #[test]
    fn fixed_seed_is_deterministic() {
        let first = plan_three_samples(&track(), &[7; 32], 5_000).unwrap();
        let second = plan_three_samples(&track(), &[7; 32], 5_000).unwrap();
        assert_eq!(first, second);
        assert_ne!(
            first,
            plan_three_samples(&track(), &[8; 32], 5_000).unwrap()
        );
    }

    #[test]
    fn rejects_preview_longer_than_a_bucket() {
        assert_eq!(
            plan_three_samples(&track(), &[0; 32], 11_000),
            Err(SamplingError::BucketTooShort(0))
        );
    }
}
