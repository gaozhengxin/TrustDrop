use std::{env, fs, path::Path};

use drop_lib::{
    cid::compute_ipfs_cid,
    video_sampling::{
        SamplingSeedInput, derive_sampling_seed, parse_mp4_video_track,
        plan_three_samples, sampling_spec_hash,
    },
};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args().nth(1).ok_or("missing input MP4")?;
    let output = env::args().nth(2).ok_or("missing output directory")?;
    let data = fs::read(&input)?;
    let track = parse_mp4_video_track(&data).map_err(|error| format!("{error:?}"))?;

    let origin_digest: [u8; 32] = Sha256::digest(&data).into();
    let spec_hash = sampling_spec_hash(5_000).map_err(|error| format!("{error:?}"))?;
    let seed = derive_sampling_seed(&SamplingSeedInput {
        chain_id: 0,
        sale_contract: [0; 20],
        sale_id: [0; 32],
        origin_blob_id: origin_digest,
        spec_hash,
        external_randomness: [0; 32],
    });
    let plans = plan_three_samples(&track, &seed, 5_000)
        .map_err(|error| format!("{error:?}"))?;

    fs::create_dir_all(&output)?;
    let mut entries = Vec::new();
    for plan in plans {
        let last_index = track.samples.iter()
            .filter(|sample| {
                sample.index >= plan.decode_start_sample
                    && sample.presentation_time < plan.presentation_end_time
            })
            .map(|sample| sample.index)
            .max()
            .ok_or("sample window contains no frames")?;
        let mut sample_bytes = Vec::new();
        for sample in track.samples.iter().filter(|sample| {
            sample.index >= plan.decode_start_sample && sample.index <= last_index
        }) {
            let start = usize::try_from(sample.byte_offset)?;
            let end = start.checked_add(sample.byte_size as usize).ok_or("range overflow")?;
            sample_bytes.extend_from_slice(data.get(start..end).ok_or("range outside MP4")?);
        }

        let filename = format!("sample-{}.avc", plan.bucket_index);
        fs::write(Path::new(&output).join(&filename), &sample_bytes)?;
        entries.push(format!(
            "    {{\"bucket\":{},\"file\":\"{}\",\"cid\":\"{}\",\"bytes\":{},\"decode_start_sample\":{},\"decode_end_sample\":{},\"target_time\":{},\"presentation_end_time\":{}}}",
            plan.bucket_index,
            filename,
            compute_ipfs_cid(&sample_bytes),
            sample_bytes.len(),
            plan.decode_start_sample,
            last_index,
            plan.target_time,
            plan.presentation_end_time,
        ));
    }

    let manifest = format!(
        "{{\n  \"source\":\"{}\",\n  \"source_sha256\":\"{}\",\n  \"origin_binding_note\":\"source SHA-256 is a temporary stand-in for the Walrus blob ID\",\n  \"timescale\":{},\n  \"preview_duration_ms\":5000,\n  \"sampling_seed\":\"{}\",\n  \"samples\":[\n{}\n  ]\n}}\n",
        input,
        hex::encode(origin_digest),
        track.timescale,
        hex::encode(seed),
        entries.join(",\n"),
    );
    fs::write(Path::new(&output).join("manifest.json"), manifest)?;
    Ok(())
}
