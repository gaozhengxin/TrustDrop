mod certificate;

#[cfg(any(feature = "execute", feature = "network"))]
use alloy_sol_types::SolValue;
#[cfg(feature = "chain-verify")]
use alloy_sol_types::{sol, SolCall};
use drop_lib::{
    rslh_ve::{walrus_symbol_size, COL_HEIGHT_SECONDARY},
    video_sampling::{
        derive_sampling_seed, parse_mp4_video_track, plan_three_samples, sampling_spec_hash,
        SamplePlan, SamplingSeedInput, VideoTrack,
    },
    walrus_blob_id::SliverPairRoots,
};
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "execute", feature = "network"))]
use sp1_sdk::{include_elf, Prover, ProverClient, SP1Stdin};
#[cfg(feature = "network")]
use sp1_sdk::{network::NetworkMode, HashableKey, ProveRequest, ProvingKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    num::NonZeroU16,
    path::{Path, PathBuf},
    process::Command,
};
#[cfg(any(feature = "execute", feature = "network"))]
use video_sampling_lib::VideoSamplingPublicValues;
use walrus_core::{
    encoding::{EncodingConfig, EncodingFactory as _, SliverPair},
    fastcrypto::Blake2b256,
    merkle::{MerkleTree, Node},
    metadata::BlobMetadataApi as _,
    EncodingType,
};

#[cfg(any(feature = "execute", feature = "network"))]
const VIDEO_SAMPLING_ELF: sp1_sdk::Elf = include_elf!("video-sampling-program");
const PREVIEW_DURATION_MS: u32 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TopLevelBox {
    offset: u64,
    size: u64,
    kind: [u8; 4],
    header_size: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VideoSamplingWitness {
    origin: WalrusVideoOpening,
    top_level_boxes: Vec<TopLevelBox>,
    previews: [PreviewTemplate; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PreviewTemplate {
    file_len: u64,
    non_sample_segments: Vec<PreviewSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PreviewSegment {
    offset: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalrusVideoOpening {
    blob_id: [u8; 32],
    encoding_type: u8,
    unencoded_length: u64,
    n_shards: u32,
    metadata_root: [u8; 32],
    primary_slivers: Vec<AuthenticatedPrimarySliver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthenticatedPrimarySliver {
    shard_index: u32,
    primary_root: [u8; 32],
    secondary_root: [u8; 32],
    pair_leaf_path: Vec<Node>,
    symbols: Vec<AuthenticatedPrimarySymbol>,
    proof_nodes: Vec<MultiproofNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthenticatedPrimarySymbol {
    leaf_index: u32,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MultiproofNode {
    level: u8,
    index: u32,
    node: Node,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SaleContext {
    chain_id: u64,
    sale_contract: [u8; 20],
    sale_id: [u8; 32],
    external_randomness: [u8; 32],
}

#[cfg(not(any(feature = "execute", feature = "network", feature = "chain-verify")))]
fn main() {
    let asset = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: video-sampling-script <input.mp4> <output-dir>"),
    );
    let output_dir = PathBuf::from(
        env::args()
            .nth(2)
            .expect("usage: video-sampling-script <input.mp4> <output-dir>"),
    );
    fs::create_dir_all(&output_dir).expect("create output directory");
    let mp4 = fs::read(&asset).expect("read source MP4");
    let source_len = mp4.len();
    let track = parse_mp4_video_track(&mp4).expect("supported source MP4");
    let boxes = parse_top_level_boxes(&mp4);

    println!("encoding {} source bytes with Walrus RS2...", source_len);
    let config =
        EncodingConfig::new(NonZeroU16::new(1000).unwrap()).get_for_type(EncodingType::RS2);
    let (slivers, metadata) = config.encode_with_metadata(mp4).expect("Walrus encoding");
    let origin_blob_id: [u8; 32] = metadata.blob_id().as_ref().try_into().unwrap();

    let sale = SaleContext {
        chain_id: 0,
        sale_contract: [0; 20],
        sale_id: [0; 32],
        external_randomness: [0; 32],
    };
    let spec_hash = sampling_spec_hash(PREVIEW_DURATION_MS).unwrap();
    let seed = derive_sampling_seed(&SamplingSeedInput {
        chain_id: sale.chain_id,
        sale_contract: sale.sale_contract,
        sale_id: sale.sale_id,
        origin_blob_id,
        spec_hash,
        external_randomness: sale.external_randomness,
    });
    let plans = plan_three_samples(&track, &seed, PREVIEW_DURATION_MS).expect("sampling plans");
    let previews = build_previews(&asset, &output_dir, &track, &plans);

    let symbol_size = walrus_symbol_size(source_len as u64);
    let mut needed = BTreeMap::<u32, BTreeSet<u32>>::new();
    for entry in &boxes {
        mark_range(
            &mut needed,
            entry.offset,
            entry.header_size as u64,
            symbol_size,
        );
        if &entry.kind == b"moov" {
            mark_range(&mut needed, entry.offset, entry.size, symbol_size);
        }
    }
    for plan in &plans {
        for sample in selected_samples(&track, plan) {
            mark_range(
                &mut needed,
                sample.byte_offset,
                sample.byte_size as u64,
                symbol_size,
            );
        }
    }
    let opening = build_opening_from_slivers(&slivers, &metadata, &config, needed);
    let witness = VideoSamplingWitness {
        origin: opening,
        top_level_boxes: boxes,
        previews,
    };
    let witness_path = output_dir.join("video-sampling-witness.bin");
    fs::write(&witness_path, bincode::serialize(&(witness, sale)).unwrap()).unwrap();
    println!("witness: {}", witness_path.display());
}

#[cfg(any(
    all(feature = "execute", feature = "network"),
    all(feature = "execute", feature = "chain-verify"),
    all(feature = "network", feature = "chain-verify")
))]
compile_error!("features `execute`, `network`, and `chain-verify` are mutually exclusive");

#[cfg(feature = "execute")]
#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    let witness_path = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: video-sampling-script <witness.bin>"),
    );
    let encoded = fs::read(&witness_path).expect("read witness");
    let (witness, sale): (VideoSamplingWitness, SaleContext) =
        bincode::deserialize(&encoded).expect("decode witness");
    let origin_blob_id = witness.origin.blob_id;
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    stdin.write(&sale);
    let client = ProverClient::builder().cpu().build().await;
    println!("executing authenticated video-sampling guest...");
    let (public_values, report) = client
        .execute(VIDEO_SAMPLING_ELF, stdin)
        .await
        .expect("SP1 guest execute");
    let values = VideoSamplingPublicValues::abi_decode(public_values.as_slice())
        .expect("decode public values");
    assert_eq!(<[u8; 32]>::from(values.originBlobId), origin_blob_id);
    println!("originBlobId: 0x{}", hex::encode(origin_blob_id));
    println!(
        "previewCidDigest0: 0x{}",
        hex::encode(values.previewCidDigest0)
    );
    println!(
        "previewCidDigest1: 0x{}",
        hex::encode(values.previewCidDigest1)
    );
    println!(
        "previewCidDigest2: 0x{}",
        hex::encode(values.previewCidDigest2)
    );
    println!("guestCycles: {}", report.total_instruction_count());
}

#[cfg(feature = "network")]
#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    let witness_path = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: video-sampling-script <witness.bin> <proof.json>"),
    );
    let proof_path = PathBuf::from(
        env::args()
            .nth(2)
            .expect("usage: video-sampling-script <witness.bin> <proof.json>"),
    );
    let encoded = fs::read(&witness_path).expect("read witness");
    let (witness, sale): (VideoSamplingWitness, SaleContext) =
        bincode::deserialize(&encoded).expect("decode witness");
    let origin_blob_id = witness.origin.blob_id;

    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    stdin.write(&sale);

    let private_key = network_private_key();
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .private_key(&private_key)
        .build()
        .await;
    let pk = client
        .setup(VIDEO_SAMPLING_ELF)
        .await
        .expect("network setup");
    println!("programVKey: {}", pk.verifying_key().bytes32());
    println!("submitting Groth16 proof request with local simulation skipped...");
    let proof = client
        .prove(&pk, stdin)
        .skip_simulation(true)
        .compressed()
        .groth16()
        .await
        .expect("network proof");

    let values = VideoSamplingPublicValues::abi_decode(proof.public_values.as_slice())
        .expect("decode public values");
    assert_eq!(<[u8; 32]>::from(values.originBlobId), origin_blob_id);
    let fixture = serde_json::json!({
        "originBlobId": format!("0x{}", hex::encode(origin_blob_id)),
        "programVKey": pk.verifying_key().bytes32(),
        "publicValues": format!("0x{}", hex::encode(proof.public_values.as_slice())),
        "proof": format!("0x{}", hex::encode(proof.bytes())),
    });
    fs::write(&proof_path, serde_json::to_vec_pretty(&fixture).unwrap())
        .expect("write proof fixture");
    println!("networkProof: {}", proof_path.display());
}

#[cfg(feature = "network")]
fn network_private_key() -> String {
    if let Ok(value) = env::var("NETWORK_PRIVATE_KEY").or_else(|_| env::var("SP1_PRIVATE_KEY")) {
        if !value.is_empty() {
            return value;
        }
    }
    let path = env::var("SP1_PRIVATE_KEY_FILE")
        .expect("NETWORK_PRIVATE_KEY, SP1_PRIVATE_KEY, or SP1_PRIVATE_KEY_FILE must be set");
    fs::read_to_string(path)
        .expect("read SP1 private key file")
        .lines()
        .find_map(|line| line.strip_prefix("SP1_PRIVATE_KEY="))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .expect("SP1_PRIVATE_KEY is missing from key file")
}

#[cfg(feature = "chain-verify")]
sol! {
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external view;
}

#[cfg(feature = "chain-verify")]
#[tokio::main]
async fn main() {
    let fixture_path = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: video-sampling-client <proof.json>"),
    );
    let fixture: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture_path).expect("read video-sampling proof fixture"))
            .expect("decode video-sampling proof fixture");
    let decode_hex = |field: &str| {
        hex::decode(
            fixture[field]
                .as_str()
                .unwrap_or_else(|| panic!("missing fixture field: {field}"))
                .strip_prefix("0x")
                .expect("fixture hex must start with 0x"),
        )
        .expect("invalid fixture hex")
    };
    let vkey: [u8; 32] = decode_hex("programVKey")
        .try_into()
        .expect("programVKey must be 32 bytes");
    let call = verifyProofCall {
        programVKey: vkey.into(),
        publicValues: decode_hex("publicValues").into(),
        proofBytes: decode_hex("proof").into(),
    };
    let gateway = env::var("SP1_VERIFIER_GATEWAY")
        .unwrap_or_else(|_| "0x397A5f7f3dBd538f23DE225B51f532c34448dA9B".to_owned());
    let rpc = env::var("ARBITRUM_SEPOLIA_RPC_URL")
        .unwrap_or_else(|_| "https://sepolia-rollup.arbitrum.io/rpc".to_owned());
    let response: serde_json::Value = reqwest::Client::new()
        .post(rpc)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [{
                "to": gateway,
                "data": format!("0x{}", hex::encode(call.abi_encode())),
            }, "latest"],
        }))
        .send()
        .await
        .expect("send verifier eth_call")
        .error_for_status()
        .expect("verifier RPC HTTP error")
        .json()
        .await
        .expect("decode verifier RPC response");
    if let Some(error) = response.get("error") {
        panic!("SP1 gateway rejected proof: {error}");
    }
    assert_eq!(response["result"], "0x", "unexpected verifier return data");
    println!("chainVerification: accepted by {gateway}");
}

fn build_opening_from_slivers(
    slivers: &[SliverPair],
    metadata: &walrus_core::metadata::VerifiedBlobMetadataWithId,
    config: &walrus_core::encoding::EncodingConfigEnum,
    needed: BTreeMap<u32, BTreeSet<u32>>,
) -> WalrusVideoOpening {
    let roots = metadata
        .metadata()
        .hashes()
        .iter()
        .map(|pair| SliverPairRoots::new(pair.primary_hash.bytes(), pair.secondary_hash.bytes()))
        .collect::<Vec<_>>();
    let pair_tree = MerkleTree::<Blake2b256>::build(roots.iter().map(|pair| {
        let mut leaf = [0u8; 64];
        leaf[..32].copy_from_slice(&pair.primary);
        leaf[32..].copy_from_slice(&pair.secondary);
        leaf
    }));
    let metadata_root = pair_tree.root().bytes();
    let n_shards = roots.len();
    let primary_slivers = needed
        .into_iter()
        .map(|(shard_index, leaves)| {
            let index = shard_index as usize;
            let recovery = slivers[index].primary.recovery_symbols(config).unwrap();
            let expanded = recovery.to_symbols().collect::<Vec<_>>();
            let tree = MerkleTree::<Blake2b256>::build(expanded.iter().copied());
            assert_eq!(tree.root(), Node::Digest(roots[index].primary));
            let mut proof_nodes = BTreeMap::<(u8, u32), Node>::new();
            let symbols = leaves
                .into_iter()
                .map(|leaf_index| {
                    let path = tree.get_proof(leaf_index as usize).unwrap();
                    let mut level_index = leaf_index as usize;
                    let mut width = n_shards;
                    for (level, node) in path.path().iter().enumerate() {
                        let padded = width.next_multiple_of(2);
                        let sibling = if level_index.is_multiple_of(2) {
                            level_index + 1
                        } else {
                            level_index - 1
                        };
                        proof_nodes
                            .entry((level as u8, sibling as u32))
                            .or_insert_with(|| node.clone());
                        level_index /= 2;
                        width = padded / 2;
                    }
                    AuthenticatedPrimarySymbol {
                        leaf_index,
                        bytes: expanded[leaf_index as usize].to_vec(),
                    }
                })
                .collect();
            AuthenticatedPrimarySliver {
                shard_index,
                primary_root: roots[index].primary,
                secondary_root: roots[index].secondary,
                pair_leaf_path: pair_tree.get_proof(index).unwrap().path().to_vec(),
                symbols,
                proof_nodes: proof_nodes
                    .into_iter()
                    .map(|((level, index), node)| MultiproofNode { level, index, node })
                    .collect(),
            }
        })
        .collect();
    WalrusVideoOpening {
        blob_id: metadata.blob_id().0,
        encoding_type: metadata.metadata().encoding_type().into(),
        unencoded_length: metadata.metadata().unencoded_length(),
        n_shards: n_shards as u32,
        metadata_root,
        primary_slivers,
    }
}

fn build_previews(
    asset: &Path,
    output_dir: &Path,
    track: &VideoTrack,
    plans: &[SamplePlan; 3],
) -> [PreviewTemplate; 3] {
    let mut previews: [PreviewTemplate; 3] = std::array::from_fn(|_| PreviewTemplate {
        file_len: 0,
        non_sample_segments: Vec::new(),
    });
    for plan in plans {
        let path = output_dir.join(format!("preview-{}.mp4", plan.bucket_index));
        let start = plan.decode_start_time as f64 / track.timescale as f64;
        let duration =
            (plan.presentation_end_time - plan.decode_start_time) as f64 / track.timescale as f64;
        let status = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
            .arg(format!("{start:.6}"))
            .arg("-i")
            .arg(asset)
            .args(["-t"])
            .arg(format!("{duration:.6}"))
            .args([
                "-map",
                "0:v:0",
                "-c:v",
                "copy",
                "-an",
                "-avoid_negative_ts",
                "make_zero",
                "-movflags",
                "+faststart",
            ])
            .arg(&path)
            .status()
            .expect("run ffmpeg");
        assert!(status.success(), "ffmpeg preview failed");
        let preview = fs::read(path).expect("read preview");
        previews[plan.bucket_index as usize] = strip_preview_samples(&preview);
    }
    previews
}

fn strip_preview_samples(preview: &[u8]) -> PreviewTemplate {
    let track = parse_mp4_video_track(preview).expect("parse generated preview");
    let mut ranges = track
        .samples
        .iter()
        .map(|sample| {
            let start = sample.byte_offset as usize;
            let end = start + sample.byte_size as usize;
            assert!(
                end <= preview.len(),
                "generated preview sample outside file"
            );
            (start, end)
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "generated preview samples overlap");
    }

    let mut non_sample_segments = Vec::new();
    let mut cursor = 0usize;
    for (start, end) in ranges {
        if cursor < start {
            non_sample_segments.push(PreviewSegment {
                offset: cursor as u64,
                bytes: preview[cursor..start].to_vec(),
            });
        }
        cursor = end;
    }
    if cursor < preview.len() {
        non_sample_segments.push(PreviewSegment {
            offset: cursor as u64,
            bytes: preview[cursor..].to_vec(),
        });
    }
    PreviewTemplate {
        file_len: preview.len() as u64,
        non_sample_segments,
    }
}

fn selected_samples<'a>(
    track: &'a VideoTrack,
    plan: &SamplePlan,
) -> Vec<&'a drop_lib::video_sampling::VideoSample> {
    let last = track
        .samples
        .iter()
        .filter(|sample| {
            sample.index >= plan.decode_start_sample
                && sample.presentation_time < plan.presentation_end_time
        })
        .map(|sample| sample.index)
        .max()
        .expect("nonempty sample window");
    track
        .samples
        .iter()
        .filter(|sample| sample.index >= plan.decode_start_sample && sample.index <= last)
        .collect()
}

fn mark_range(
    needed: &mut BTreeMap<u32, BTreeSet<u32>>,
    offset: u64,
    len: u64,
    symbol_size: usize,
) {
    let symbol_size = symbol_size as u64;
    let first = offset / symbol_size;
    let last = (offset + len - 1) / symbol_size;
    for flat in first..=last {
        let shard = (flat / COL_HEIGHT_SECONDARY as u64) as u32;
        let leaf = (flat % COL_HEIGHT_SECONDARY as u64) as u32;
        needed.entry(shard).or_default().insert(leaf);
    }
}

fn parse_top_level_boxes(data: &[u8]) -> Vec<TopLevelBox> {
    let mut boxes = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        assert!(offset + 8 <= data.len(), "truncated top-level box");
        let size32 = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
        let kind = data[offset + 4..offset + 8].try_into().unwrap();
        let (header_size, size) = if size32 == 1 {
            assert!(offset + 16 <= data.len(), "truncated extended box");
            (
                16u8,
                u64::from_be_bytes(data[offset + 8..offset + 16].try_into().unwrap()),
            )
        } else if size32 == 0 {
            (8u8, (data.len() - offset) as u64)
        } else {
            (8u8, size32 as u64)
        };
        assert!(size >= header_size as u64 && offset as u64 + size <= data.len() as u64);
        boxes.push(TopLevelBox {
            offset: offset as u64,
            size,
            kind,
            header_size,
        });
        offset += size as usize;
    }
    boxes
}
