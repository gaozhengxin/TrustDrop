#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolValue;
use drop_lib::{
    cid::compute_ipfs_cid_zk_optimized,
    rslh_ve::{walrus_symbol_size, COL_HEIGHT_SECONDARY},
    video_sampling::{
        derive_sampling_seed, parse_mp4_video_track, parse_mp4_video_track_from_moov,
        plan_three_samples, sampling_spec_hash, SamplingSeedInput, VideoSample,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use video_sampling_lib::VideoSamplingPublicValues;
use walrus_core::{
    fastcrypto::Blake2b256,
    merkle::{MerkleAuth, MerkleProof, MerkleTree, Node},
    BlobId, EncodingType,
};

const PREVIEW_DURATION_MS: u32 = 5_000;

/// A top-level MP4 box directory supplied by the host. Every header is read back
/// from authenticated Walrus symbols before the directory is trusted.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TopLevelBox {
    offset: u64,
    size: u64,
    kind: [u8; 4],
    header_size: u8,
}

/// Private witness. Large media bytes live only in Walrus symbol openings;
/// previews are untrusted candidates whose complete bytes are checked below.
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

pub fn main() {
    let witness = sp1_zkvm::io::read::<VideoSamplingWitness>();
    let sale = sp1_zkvm::io::read::<SaleContext>();

    // First bind every supplied source symbol to one Walrus blob ID.
    verify_walrus_video_opening(&witness.origin);
    let moov = authenticated_moov(&witness);
    let source_track = parse_mp4_video_track_from_moov(&moov, witness.origin.unencoded_length)
        .expect("unsupported authenticated MP4 metadata");

    // The claimant supplies randomness, but cannot supply the resulting sample
    // positions: all three positions are derived again inside the guest.
    let spec_hash = sampling_spec_hash(PREVIEW_DURATION_MS).expect("valid sampling spec");
    let sampling_seed = derive_sampling_seed(&SamplingSeedInput {
        chain_id: sale.chain_id,
        sale_contract: sale.sale_contract,
        sale_id: sale.sale_id,
        origin_blob_id: witness.origin.blob_id,
        spec_hash,
        external_randomness: sale.external_randomness,
    });
    let plans = plan_three_samples(&source_track, &sampling_seed, PREVIEW_DURATION_MS)
        .expect("unable to plan authenticated video samples");

    let mut preview_cid_digests = [[0u8; 32]; 3];
    for plan in plans {
        let selected = selected_samples(&source_track.samples, &plan);
        let preview = materialize_preview(
            &witness.previews[plan.bucket_index as usize],
            &source_track.codec,
            &selected,
            &witness.origin,
        );

        let cid = compute_ipfs_cid_zk_optimized(&preview);
        preview_cid_digests[plan.bucket_index as usize].copy_from_slice(&cid[cid.len() - 32..]);
    }

    let values = VideoSamplingPublicValues {
        originBlobId: witness.origin.blob_id.into(),
        specHash: spec_hash.into(),
        samplingSeed: sampling_seed.into(),
        previewCidDigest0: preview_cid_digests[0].into(),
        previewCidDigest1: preview_cid_digests[1].into(),
        previewCidDigest2: preview_cid_digests[2].into(),
    };
    sp1_zkvm::io::commit_slice(&values.abi_encode());
}

fn verify_walrus_video_opening(origin: &WalrusVideoOpening) {
    let encoding = EncodingType::try_from(origin.encoding_type).expect("invalid Walrus encoding");
    assert_eq!(origin.n_shards, 1000, "unsupported Walrus shard count");
    let blob_id = BlobId::from_metadata(
        Node::Digest(origin.metadata_root),
        encoding,
        origin.unencoded_length,
    );
    assert_eq!(blob_id.0, origin.blob_id, "Walrus blob ID mismatch");
    let symbol_size = walrus_symbol_size(origin.unencoded_length);
    let mut seen_shards = BTreeSet::new();
    for sliver in &origin.primary_slivers {
        assert!(
            seen_shards.insert(sliver.shard_index),
            "duplicate primary sliver"
        );
        assert!((sliver.shard_index as usize) < origin.n_shards as usize);
        let mut pair_leaf = [0u8; 64];
        pair_leaf[..32].copy_from_slice(&sliver.primary_root);
        pair_leaf[32..].copy_from_slice(&sliver.secondary_root);
        MerkleProof::<Blake2b256>::new(&sliver.pair_leaf_path)
            .verify_proof(
                &Node::Digest(origin.metadata_root),
                origin.n_shards as usize,
                &pair_leaf,
                sliver.shard_index as usize,
            )
            .expect("sliver pair opening mismatch");
        assert_eq!(
            verify_primary_multiproof(sliver, origin.n_shards as usize, symbol_size),
            Node::Digest(sliver.primary_root),
            "primary symbol multiproof mismatch"
        );
    }
}

fn verify_primary_multiproof(
    sliver: &AuthenticatedPrimarySliver,
    n_leaves: usize,
    symbol_size: usize,
) -> Node {
    let mut current = BTreeMap::<usize, Node>::new();
    for symbol in &sliver.symbols {
        assert_eq!(symbol.bytes.len(), symbol_size, "wrong Walrus symbol size");
        let index = symbol.leaf_index as usize;
        assert!(
            index < n_leaves
                && current
                    .insert(
                        index,
                        MerkleTree::<Blake2b256>::build(core::iter::once(symbol.bytes.as_slice()))
                            .root(),
                    )
                    .is_none(),
            "duplicate primary symbol"
        );
    }
    assert!(!current.is_empty(), "empty primary symbol multiproof");
    let proof = sliver
        .proof_nodes
        .iter()
        .map(|item| {
            (
                (item.level as usize, item.index as usize),
                item.node.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut level = 0usize;
    let mut width = n_leaves;
    while width > 1 {
        let padded = width.next_multiple_of(2);
        let parents = current
            .keys()
            .map(|index| index / 2)
            .collect::<BTreeSet<_>>();
        let mut next = BTreeMap::new();
        for parent in parents {
            let child = |index: usize| {
                current
                    .get(&index)
                    .cloned()
                    .or_else(|| proof.get(&(level, index)).cloned())
                    .or_else(|| (index >= width).then_some(Node::Empty))
                    .expect("missing primary multiproof node")
            };
            let root = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
                [child(parent * 2), child(parent * 2 + 1)].into_iter(),
            )
            .root();
            next.insert(parent, root);
        }
        current = next;
        width = padded / 2;
        level += 1;
    }
    current.remove(&0).expect("multiproof produced no root")
}

fn authenticated_moov(witness: &VideoSamplingWitness) -> Vec<u8> {
    let mut cursor = 0u64;
    let mut moov_payload = None;
    for entry in &witness.top_level_boxes {
        assert_eq!(entry.offset, cursor, "top-level MP4 directory has a gap");
        assert!(
            entry.header_size == 8 || entry.header_size == 16,
            "invalid MP4 header size"
        );
        assert!(
            entry.size >= entry.header_size as u64,
            "invalid top-level MP4 box size"
        );

        let header = read_origin_range(&witness.origin, entry.offset, entry.header_size as u64);
        assert_eq!(&header[4..8], &entry.kind, "top-level MP4 type mismatch");
        let declared = if entry.header_size == 8 {
            u32::from_be_bytes(header[0..4].try_into().unwrap()) as u64
        } else {
            assert_eq!(u32::from_be_bytes(header[0..4].try_into().unwrap()), 1);
            u64::from_be_bytes(header[8..16].try_into().unwrap())
        };
        assert_eq!(declared, entry.size, "top-level MP4 size mismatch");
        assert_ne!(&entry.kind, b"moof", "fragmented MP4 is unsupported");

        if &entry.kind == b"moov" {
            assert!(moov_payload.is_none(), "multiple moov boxes");
            moov_payload = Some(read_origin_range(
                &witness.origin,
                entry.offset + entry.header_size as u64,
                entry.size - entry.header_size as u64,
            ));
        }
        cursor = cursor
            .checked_add(entry.size)
            .expect("MP4 directory overflow");
    }
    assert_eq!(
        cursor, witness.origin.unencoded_length,
        "MP4 directory length mismatch"
    );
    moov_payload.expect("missing moov box")
}

fn selected_samples<'a>(
    samples: &'a [VideoSample],
    plan: &drop_lib::video_sampling::SamplePlan,
) -> Vec<&'a VideoSample> {
    let last_index = samples
        .iter()
        .filter(|sample| {
            sample.index >= plan.decode_start_sample
                && sample.presentation_time < plan.presentation_end_time
        })
        .map(|sample| sample.index)
        .max()
        .expect("sample window contains no frames");
    samples
        .iter()
        .filter(|sample| sample.index >= plan.decode_start_sample && sample.index <= last_index)
        .collect()
}

fn materialize_preview(
    template: &PreviewTemplate,
    source_codec: &[u8; 4],
    source_samples: &[&VideoSample],
    origin: &WalrusVideoOpening,
) -> Vec<u8> {
    let file_len = usize::try_from(template.file_len).expect("preview too large");
    let mut preview = vec![0u8; file_len];
    let mut supplied_segments = Vec::with_capacity(template.non_sample_segments.len());
    let mut previous_end = 0usize;
    for segment in &template.non_sample_segments {
        let start = usize::try_from(segment.offset).expect("preview segment offset overflow");
        let end = start
            .checked_add(segment.bytes.len())
            .expect("preview segment overflow");
        assert!(
            start >= previous_end && end <= file_len,
            "invalid preview segment"
        );
        preview[start..end].copy_from_slice(&segment.bytes);
        supplied_segments.push((start, end));
        previous_end = end;
    }

    // MP4 metadata lives outside the stripped sample payloads, so the parser
    // can recover the exact sample layout from this zero-filled skeleton.
    let preview_track = parse_mp4_video_track(&preview).expect("invalid preview MP4 template");
    assert_eq!(&preview_track.codec, source_codec, "preview codec changed");
    assert_eq!(
        preview_track.samples.len(),
        source_samples.len(),
        "preview sample count changed"
    );

    let sample_ranges = sorted_sample_ranges(&preview_track.samples, file_len);
    assert_eq!(
        supplied_segments,
        complement_ranges(&sample_ranges, file_len),
        "preview template must contain exactly the non-sample bytes"
    );

    for (preview_sample, source_sample) in preview_track.samples.iter().zip(source_samples) {
        assert_eq!(
            preview_sample.byte_size, source_sample.byte_size,
            "preview sample size changed"
        );
        assert_eq!(
            preview_sample.duration, source_sample.duration,
            "preview sample duration changed"
        );
        assert_eq!(
            preview_sample.is_sync, source_sample.is_sync,
            "preview sync table changed"
        );

        let preview_start = preview_sample.byte_offset as usize;
        let preview_end = preview_start + preview_sample.byte_size as usize;
        let source_bytes = read_origin_range(
            origin,
            source_sample.byte_offset,
            source_sample.byte_size as u64,
        );
        preview[preview_start..preview_end].copy_from_slice(&source_bytes);
    }
    preview
}

fn sorted_sample_ranges(samples: &[VideoSample], file_len: usize) -> Vec<(usize, usize)> {
    let mut ranges = samples
        .iter()
        .map(|sample| {
            let start = usize::try_from(sample.byte_offset).expect("sample offset overflow");
            let end = start
                .checked_add(sample.byte_size as usize)
                .expect("sample range overflow");
            assert!(end <= file_len, "sample outside preview");
            (start, end)
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        assert!(pair[0].1 <= pair[1].0, "overlapping preview samples");
    }
    ranges
}

fn complement_ranges(sample_ranges: &[(usize, usize)], file_len: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut cursor = 0usize;
    for &(start, end) in sample_ranges {
        if cursor < start {
            result.push((cursor, start));
        }
        cursor = end;
    }
    if cursor < file_len {
        result.push((cursor, file_len));
    }
    result
}

/// Walrus RS2 primary symbols contain the systematic source matrix. For the
/// standard 1000-shard layout, flat source symbol `k` is row `k / 667`, leaf
/// `k % 667`. The Merkle verification above authenticates every symbol used here.
fn read_origin_range(origin: &WalrusVideoOpening, offset: u64, len: u64) -> Vec<u8> {
    let end = offset.checked_add(len).expect("source range overflow");
    assert!(end <= origin.unencoded_length, "source range outside blob");
    let symbol_size = walrus_symbol_size(origin.unencoded_length) as u64;
    let mut cursor = offset;
    let mut output = Vec::with_capacity(len as usize);
    while cursor < end {
        let flat_symbol = cursor / symbol_size;
        let shard_index = flat_symbol / COL_HEIGHT_SECONDARY as u64;
        let leaf_index = flat_symbol % COL_HEIGHT_SECONDARY as u64;
        let symbol = find_primary_symbol(origin, shard_index as u32, leaf_index as u32);
        assert_eq!(
            symbol.len(),
            symbol_size as usize,
            "wrong Walrus symbol size"
        );

        let within = (cursor % symbol_size) as usize;
        let take = usize::min(symbol.len() - within, (end - cursor) as usize);
        output.extend_from_slice(&symbol[within..within + take]);
        cursor += take as u64;
    }
    output
}

fn find_primary_symbol(origin: &WalrusVideoOpening, shard_index: u32, leaf_index: u32) -> &[u8] {
    let shard = origin
        .primary_slivers
        .iter()
        .find(|shard| shard.shard_index == shard_index)
        .expect("missing authenticated primary sliver");
    &shard
        .symbols
        .iter()
        .find(|symbol| symbol.leaf_index == leaf_index)
        .expect("missing authenticated primary symbol")
        .bytes
}
