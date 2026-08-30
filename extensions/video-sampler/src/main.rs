use std::{collections::HashMap, env, fs, path::Path, process::Command};

use drop_lib::{
    cid::compute_ipfs_cid,
    video_sampling::{
        SamplingSeedInput, derive_sampling_seed, parse_mp4_video_track, plan_three_samples,
        sampling_spec_hash,
    },
};
use sha2::{Digest, Sha256};

const PREVIEW_DURATION_MS: u32 = 5_000;
const PINATA_UPLOAD_URL: &str = "https://api.pinata.cloud/pinning/pinFileToIPFS";

enum PinataAuth {
    Jwt(String),
    ApiKey { key: String, secret: String },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .ok_or("usage: trustdrop-video-sampler <input.mp4> <output-dir>")?;
    let output = env::args()
        .nth(2)
        .ok_or("usage: trustdrop-video-sampler <input.mp4> <output-dir>")?;
    let input = fs::canonicalize(input)?;
    let data = fs::read(&input)?;
    let track =
        parse_mp4_video_track(&data).map_err(|error| format!("MP4 parse failed: {error:?}"))?;

    let source_sha256: [u8; 32] = Sha256::digest(&data).into();
    let spec_hash =
        sampling_spec_hash(PREVIEW_DURATION_MS).map_err(|error| format!("{error:?}"))?;
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
    let pinata = if env::var("PINATA_UPLOAD").as_deref() == Ok("0") {
        None
    } else {
        Some(load_pinata_auth()?)
    };
    let mut manifest_entries = Vec::new();
    let mut preview_files = Vec::new();
    for plan in plans {
        let last_index = track
            .samples
            .iter()
            .filter(|sample| {
                sample.index >= plan.decode_start_sample
                    && sample.presentation_time < plan.presentation_end_time
            })
            .map(|sample| sample.index)
            .max()
            .ok_or("sample window contains no frames")?;
        let mut evidence = Vec::new();
        for sample in track
            .samples
            .iter()
            .filter(|sample| sample.index >= plan.decode_start_sample && sample.index <= last_index)
        {
            let start = usize::try_from(sample.byte_offset)?;
            let end = start
                .checked_add(sample.byte_size as usize)
                .ok_or("sample range overflow")?;
            evidence.extend_from_slice(data.get(start..end).ok_or("sample range outside MP4")?);
        }

        let preview_name = format!("preview-{}.mp4", plan.bucket_index);
        let preview_path = Path::new(&output).join(&preview_name);
        let start_seconds = plan.decode_start_time as f64 / track.timescale as f64;
        let duration_seconds =
            (plan.presentation_end_time - plan.decode_start_time) as f64 / track.timescale as f64;
        let status = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-ss", &format!("{start_seconds:.6}")])
            .args(["-i", input.to_str().ok_or("non-UTF8 input path")?])
            .args(["-t", &format!("{duration_seconds:.6}")])
            .args(["-map", "0:v:0", "-c:v", "copy", "-an"])
            .args(["-avoid_negative_ts", "make_zero", "-movflags", "+faststart"])
            .arg(preview_path.to_str().ok_or("non-UTF8 output path")?)
            .status()?;
        if !status.success() {
            return Err(format!("ffmpeg failed for bucket {}", plan.bucket_index).into());
        }

        let preview_bytes = fs::read(&preview_path)?;
        let preview_cid = compute_ipfs_cid(&preview_bytes);
        let ipfs_url = if let Some(auth) = &pinata {
            let returned_cid = upload_to_pinata(auth, &preview_path, &preview_name)?;
            if returned_cid != preview_cid {
                return Err(format!(
                    "Pinata CID mismatch for {preview_name}: local={preview_cid}, returned={returned_cid}"
                )
                .into());
            }
            Some(format!("ipfs://{returned_cid}"))
        } else {
            None
        };

        preview_files.push(preview_name.clone());
        manifest_entries.push(format!(
            "    {{\"bucket\":{},\"preview\":\"{}\",\"preview_cid\":\"{}\",\"ipfs_url\":{},\"evidence_cid\":\"{}\",\"evidence_bytes\":{},\"decode_start_sample\":{},\"decode_end_sample\":{},\"target_time\":{},\"presentation_end_time\":{}}}",
            plan.bucket_index, preview_name, preview_cid,
            serde_json::to_string(&ipfs_url)?, compute_ipfs_cid(&evidence), evidence.len(),
            plan.decode_start_sample, last_index, plan.target_time, plan.presentation_end_time,
        ));
    }

    fs::write(
        Path::new(&output).join("manifest.json"),
        format!(
            "{{\n  \"source\":\"{}\",\n  \"source_sha256\":\"{}\",\n  \"binding_note\":\"source SHA-256 temporarily stands in for the Walrus blob ID\",\n  \"timescale\":{},\n  \"preview_duration_ms\":{},\n  \"sampling_seed\":\"{}\",\n  \"samples\":[\n{}\n  ]\n}}\n",
            input.display(),
            hex::encode(source_sha256),
            track.timescale,
            PREVIEW_DURATION_MS,
            hex::encode(seed),
            manifest_entries.join(",\n"),
        ),
    )?;
    fs::write(
        Path::new(&output).join("preview.html"),
        preview_html(&preview_files),
    )?;

    println!("source: {}", input.display());
    println!("seed: {}", hex::encode(seed));
    println!("preview: {}/preview.html", output);
    println!("manifest: {}/manifest.json", output);
    Ok(())
}

fn load_pinata_auth() -> Result<PinataAuth, Box<dyn std::error::Error>> {
    let mut values = HashMap::new();
    if let Ok(path) = env::var("PINATA_CONFIG_FILE") {
        for line in fs::read_to_string(path)?.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or("invalid Pinata config line")?;
            values.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    let get = |name: &str| env::var(name).ok().or_else(|| values.get(name).cloned());
    if let Some(jwt) = get("PINATA_JWT") {
        return Ok(PinataAuth::Jwt(jwt));
    }
    match (get("PINATA_API_KEY"), get("PINATA_SECRET_API_KEY")) {
        (Some(key), Some(secret)) => Ok(PinataAuth::ApiKey { key, secret }),
        (Some(_), None) => Err("PINATA_SECRET_API_KEY is required with PINATA_API_KEY".into()),
        _ => Err("set PINATA_JWT or PINATA_API_KEY plus PINATA_SECRET_API_KEY".into()),
    }
}

fn upload_to_pinata(
    auth: &PinataAuth,
    path: &Path,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let file = reqwest::blocking::multipart::Part::bytes(fs::read(path)?)
        .file_name(name.to_owned())
        .mime_str("video/mp4")?;
    let form = reqwest::blocking::multipart::Form::new()
        .part("file", file)
        .text(
            "pinataMetadata",
            serde_json::json!({ "name": name }).to_string(),
        )
        .text(
            "pinataOptions",
            serde_json::json!({ "cidVersion": 1 }).to_string(),
        );
    let client = reqwest::blocking::Client::new();
    let request = client.post(PINATA_UPLOAD_URL).multipart(form);
    let request = match auth {
        PinataAuth::Jwt(jwt) => request.bearer_auth(jwt),
        PinataAuth::ApiKey { key, secret } => request
            .header("pinata_api_key", key)
            .header("pinata_secret_api_key", secret),
    };
    let response = request.send()?.error_for_status()?;
    let body: serde_json::Value = response.json()?;
    body["IpfsHash"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "Pinata response is missing IpfsHash".into())
}

fn preview_html(files: &[String]) -> String {
    let videos = files.iter().enumerate().map(|(index, file)| format!(
        "<section><h2>Sample {index}</h2><video controls preload=\"metadata\" src=\"{file}\"></video></section>"
    )).collect::<Vec<_>>().join("\n");
    format!(
        r#"<!doctype html>
<meta charset="utf-8">
<title>TrustDrop video sampling preview</title>
<style>body{{font:16px system-ui;margin:32px;background:#111;color:#eee}}main{{display:grid;gap:24px}}video{{max-width:720px;width:100%}}</style>
<h1>TrustDrop deterministic video samples</h1><main>{videos}</main>
"#
    )
}
