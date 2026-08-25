use std::{env, fs, path::Path, process::Command};

use drop_lib::{
    cid::compute_ipfs_cid,
    video_sampling::{
        SamplingSeedInput, derive_sampling_seed, parse_mp4_video_track,
        plan_three_samples, sampling_spec_hash,
    },
};
use sha2::{Digest, Sha256};

const PREVIEW_DURATION_MS: u32 = 5_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args().nth(1).ok_or("usage: trustdrop-video-sampler <input.mp4> <output-dir>")?;
    let output = env::args().nth(2).ok_or("usage: trustdrop-video-sampler <input.mp4> <output-dir>")?;
    let input = fs::canonicalize(input)?;
    let data = fs::read(&input)?;
    let track = parse_mp4_video_track(&data).map_err(|error| format!("MP4 parse failed: {error:?}"))?;

    let source_sha256: [u8; 32] = Sha256::digest(&data).into();
    let spec_hash = sampling_spec_hash(PREVIEW_DURATION_MS).map_err(|error| format!("{error:?}"))?;
    let seed = derive_sampling_seed(&SamplingSeedInput {
        chain_id: 0,
        sale_contract: [0; 20],
        sale_id: [0; 32],
        origin_blob_id: source_sha256,
        spec_hash,
        external_randomness: [0; 32],
    });
    let plans = plan_three_samples(&track, &seed, PREVIEW_DURATION_MS)
        .map_err(|error| format!("sampling failed: {error:?}"))?;

    fs::create_dir_all(&output)?;
    let mut manifest_entries = Vec::new();
    let mut preview_files = Vec::new();
    for plan in plans {
        let last_index = track.samples.iter()
            .filter(|sample| sample.index >= plan.decode_start_sample
                && sample.presentation_time < plan.presentation_end_time)
            .map(|sample| sample.index)
            .max()
            .ok_or("sample window contains no frames")?;
        let mut evidence = Vec::new();
        for sample in track.samples.iter().filter(|sample| sample.index >= plan.decode_start_sample
            && sample.index <= last_index) {
            let start = usize::try_from(sample.byte_offset)?;
            let end = start.checked_add(sample.byte_size as usize).ok_or("sample range overflow")?;
            evidence.extend_from_slice(data.get(start..end).ok_or("sample range outside MP4")?);
        }

        let preview_name = format!("preview-{}.mp4", plan.bucket_index);
        let preview_path = Path::new(&output).join(&preview_name);
        let start_seconds = plan.decode_start_time as f64 / track.timescale as f64;
        let duration_seconds = (plan.presentation_end_time - plan.decode_start_time) as f64
            / track.timescale as f64;
        let status = Command::new("/usr/bin/avconvert")
            .args(["--source", input.to_str().ok_or("non-UTF8 input path")?])
            .args(["--output", preview_path.to_str().ok_or("non-UTF8 output path")?])
            .args(["--preset", "PresetPassthrough", "--replace"])
            .args(["--start", &format!("{start_seconds:.6}")])
            .args(["--duration", &format!("{duration_seconds:.6}")])
            .status()?;
        if !status.success() {
            return Err(format!("avconvert failed for bucket {}", plan.bucket_index).into());
        }

        preview_files.push(preview_name.clone());
        manifest_entries.push(format!(
            "    {{\"bucket\":{},\"preview\":\"{}\",\"evidence_cid\":\"{}\",\"evidence_bytes\":{},\"decode_start_sample\":{},\"decode_end_sample\":{},\"target_time\":{},\"presentation_end_time\":{}}}",
            plan.bucket_index, preview_name, compute_ipfs_cid(&evidence), evidence.len(),
            plan.decode_start_sample, last_index, plan.target_time, plan.presentation_end_time,
        ));
    }

    fs::write(Path::new(&output).join("manifest.json"), format!(
        "{{\n  \"source\":\"{}\",\n  \"source_sha256\":\"{}\",\n  \"binding_note\":\"source SHA-256 temporarily stands in for the Walrus blob ID\",\n  \"timescale\":{},\n  \"preview_duration_ms\":{},\n  \"sampling_seed\":\"{}\",\n  \"samples\":[\n{}\n  ]\n}}\n",
        input.display(), hex::encode(source_sha256), track.timescale, PREVIEW_DURATION_MS,
        hex::encode(seed), manifest_entries.join(",\n"),
    ))?;
    fs::write(Path::new(&output).join("preview.html"), preview_html(&preview_files))?;

    println!("source: {}", input.display());
    println!("seed: {}", hex::encode(seed));
    println!("preview: {}/preview.html", output);
    println!("manifest: {}/manifest.json", output);
    Ok(())
}

fn preview_html(files: &[String]) -> String {
    let videos = files.iter().enumerate().map(|(index, file)| format!(
        "<section><h2>Sample {index}</h2><video controls preload=\"metadata\" src=\"{file}\"></video></section>"
    )).collect::<Vec<_>>().join("\n");
    format!(r#"<!doctype html>
<meta charset="utf-8">
<title>TrustDrop video sampling preview</title>
<style>body{{font:16px system-ui;margin:32px;background:#111;color:#eee}}main{{display:grid;gap:24px}}video{{max-width:720px;width:100%}}</style>
<h1>TrustDrop deterministic video samples</h1><main>{videos}</main>
"#)
}
