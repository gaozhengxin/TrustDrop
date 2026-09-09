#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolValue;
use drop_lib::{
    cid::compute_ipfs_cid_zk_optimized,
    rslh_ve::{walrus_symbol_size, COL_HEIGHT_SECONDARY},
};
use flow_graph_sampling_guest_lib::{
    AuthenticatedPrimarySliver, FlowGraphSamplingPublicValues, FlowSamplingWitness, MultiproofNode,
    WalrusFlowOpening, ARCHIVE_ENTRY_LEN, ARCHIVE_HEADER_LEN, ARCHIVE_MAGIC,
};
use std::collections::{BTreeMap, BTreeSet};
use trustdrop_flow_graph_sampling::{derive_time_window, TimeCoverage};
use walrus_core::{
    fastcrypto::Blake2b256,
    merkle::{MerkleAuth, MerkleProof, MerkleTree, Node},
    BlobId, EncodingType,
};

#[derive(Debug, Clone, Copy)]
struct BucketEntry {
    bucket_start: u64,
    offset: u64,
    len: u64,
}

pub fn main() {
    let witness = sp1_zkvm::io::read::<FlowSamplingWitness>();
    let sampling_seed = sp1_zkvm::io::read::<[u8; 32]>();

    verify_walrus_opening(&witness.origin);
    let (coverage, entries, directory_end) = read_authenticated_directory(&witness.origin);
    let selected = derive_time_window(&sampling_seed, coverage).expect("invalid flow coverage");
    let entry = entries
        .iter()
        .find(|entry| entry.bucket_start == selected.bucket_start)
        .expect("selected bucket missing from authenticated directory");
    assert_eq!(entry.len > 0, true, "selected bucket is empty");

    let symbol_size = walrus_symbol_size(witness.origin.unencoded_length) as u64;
    let mut required = BTreeMap::<u32, BTreeSet<u32>>::new();
    mark_range(&mut required, 0, directory_end, symbol_size);
    mark_range(&mut required, entry.offset, entry.len, symbol_size);
    assert_exact_symbol_selection(&witness.origin, &required);

    let sample = read_origin_range(&witness.origin, entry.offset, entry.len);
    validate_sample_shape(&sample);
    let cid = compute_ipfs_cid_zk_optimized(&sample);
    let mut sample_cid_digest = [0u8; 32];
    sample_cid_digest.copy_from_slice(&cid[cid.len() - 32..]);

    let values = FlowGraphSamplingPublicValues {
        originBlobId: witness.origin.blob_id.into(),
        samplingSeed: sampling_seed.into(),
        bucketStart: selected.bucket_start,
        bucketEnd: selected.bucket_end,
        sampleCidDigest: sample_cid_digest.into(),
    };
    sp1_zkvm::io::commit_slice(&values.abi_encode());
}

fn read_authenticated_directory(
    origin: &WalrusFlowOpening,
) -> (TimeCoverage, Vec<BucketEntry>, u64) {
    let header = read_origin_range(origin, 0, ARCHIVE_HEADER_LEN);
    assert_eq!(&header[..8], ARCHIVE_MAGIC, "invalid flow archive magic");
    let bucket_seconds = read_u64(&header, 8);
    let first_timestamp = read_u64(&header, 16);
    let last_timestamp = read_u64(&header, 24);
    let entry_count = read_u32(&header, 32) as u64;
    assert!(bucket_seconds > 0, "flow bucket width must be positive");
    assert_eq!(
        read_u32(&header, 36),
        0,
        "flow archive reserved field changed"
    );
    assert!(entry_count > 0, "flow archive has no buckets");
    let directory_len = entry_count
        .checked_mul(ARCHIVE_ENTRY_LEN)
        .expect("flow directory length overflow");
    let directory_end = ARCHIVE_HEADER_LEN
        .checked_add(directory_len)
        .expect("flow directory end overflow");
    let bytes = read_origin_range(origin, ARCHIVE_HEADER_LEN, directory_len);
    let mut entries = Vec::with_capacity(entry_count as usize);
    let mut expected_offset = directory_end;
    for chunk in bytes.chunks_exact(ARCHIVE_ENTRY_LEN as usize) {
        let entry = BucketEntry {
            bucket_start: read_u64(chunk, 0),
            offset: read_u64(chunk, 8),
            len: read_u64(chunk, 16),
        };
        assert_eq!(
            entry.offset, expected_offset,
            "non-contiguous bucket payload"
        );
        assert!(entry.len > 0, "empty bucket payload");
        expected_offset = expected_offset
            .checked_add(entry.len)
            .expect("flow payload length overflow");
        entries.push(entry);
    }
    assert_eq!(
        expected_offset, origin.unencoded_length,
        "archive length mismatch"
    );
    for pair in entries.windows(2) {
        assert_eq!(
            pair[1].bucket_start,
            pair[0]
                .bucket_start
                .checked_add(bucket_seconds)
                .expect("bucket timestamp overflow"),
            "flow archive buckets are not contiguous"
        );
    }
    assert_eq!(
        first_timestamp,
        entries.first().unwrap().bucket_start,
        "coverage start mismatch"
    );
    assert_eq!(
        last_timestamp,
        entries
            .last()
            .unwrap()
            .bucket_start
            .checked_add(bucket_seconds - 1)
            .expect("coverage end overflow"),
        "coverage end mismatch"
    );
    (
        TimeCoverage {
            first_timestamp,
            last_timestamp,
            bucket_seconds,
        },
        entries,
        directory_end,
    )
}

fn validate_sample_shape(sample: &[u8]) {
    assert_eq!(
        sample.last(),
        Some(&b'\n'),
        "sample must end on a row boundary"
    );
    assert!(
        sample.iter().any(|byte| *byte != b'\n'),
        "selected bucket contains no rows"
    );
}

fn assert_exact_symbol_selection(
    origin: &WalrusFlowOpening,
    required: &BTreeMap<u32, BTreeSet<u32>>,
) {
    assert_eq!(
        origin.primary_slivers.len(),
        required.len(),
        "unexpected sliver set"
    );
    for sliver in &origin.primary_slivers {
        let expected = required
            .get(&sliver.shard_index)
            .expect("guest did not select supplied sliver");
        let supplied = sliver
            .symbols
            .iter()
            .map(|symbol| symbol.leaf_index)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            &supplied, expected,
            "unexpected symbol set in selected sliver"
        );
    }
}

fn mark_range(needed: &mut BTreeMap<u32, BTreeSet<u32>>, offset: u64, len: u64, symbol_size: u64) {
    assert!(len > 0, "cannot select an empty source range");
    let last = offset.checked_add(len - 1).expect("source range overflow");
    let first_symbol = offset / symbol_size;
    let last_symbol = last / symbol_size;
    for flat_symbol in first_symbol..=last_symbol {
        let shard = (flat_symbol / COL_HEIGHT_SECONDARY as u64) as u32;
        let leaf = (flat_symbol % COL_HEIGHT_SECONDARY as u64) as u32;
        needed.entry(shard).or_default().insert(leaf);
    }
}

fn verify_walrus_opening(origin: &WalrusFlowOpening) {
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
    // Use a dense frontier because a contiguous sample normally opens most of
    // the 667 data leaves in each touched 1000-leaf primary tree. A BTreeMap
    // frontier would pay logarithmic lookup and allocation costs per hash.
    let mut current = vec![None::<Node>; n_leaves];
    for symbol in &sliver.symbols {
        assert_eq!(symbol.bytes.len(), symbol_size, "wrong Walrus symbol size");
        let index = symbol.leaf_index as usize;
        assert!(
            index < n_leaves && current[index].is_none(),
            "duplicate primary symbol"
        );
        current[index] =
            Some(MerkleTree::<Blake2b256>::build(core::iter::once(symbol.bytes.as_slice())).root());
    }
    assert!(
        current.iter().any(Option::is_some),
        "empty primary symbol multiproof"
    );
    let mut proof = BTreeMap::new();
    for item in &sliver.proof_nodes {
        assert!(
            proof
                .insert(
                    (item.level as usize, item.index as usize),
                    item.node.clone()
                )
                .is_none(),
            "duplicate primary multiproof node"
        );
    }
    let mut level = 0usize;
    let mut width = n_leaves;
    while width > 1 {
        let padded = width.next_multiple_of(2);
        current.resize(padded, None);
        let mut next = vec![None; padded / 2];
        for parent in 0..padded / 2 {
            let left_index = parent * 2;
            let right_index = left_index + 1;
            if current[left_index].is_none() && current[right_index].is_none() {
                continue;
            }
            let mut child = |index: usize| {
                current[index]
                    .clone()
                    .or_else(|| proof.remove(&(level, index)))
                    .or_else(|| (index >= width).then_some(Node::Empty))
                    .expect("missing primary multiproof node")
            };
            let root = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
                [child(left_index), child(right_index)].into_iter(),
            )
            .root();
            next[parent] = Some(root);
        }
        current = next;
        width = padded / 2;
        level += 1;
    }
    assert!(proof.is_empty(), "unused primary multiproof nodes");
    current[0].take().expect("multiproof produced no root")
}

fn read_origin_range(origin: &WalrusFlowOpening, offset: u64, len: u64) -> Vec<u8> {
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
        let within = (cursor % symbol_size) as usize;
        let take = usize::min(symbol.len() - within, (end - cursor) as usize);
        output.extend_from_slice(&symbol[within..within + take]);
        cursor += take as u64;
    }
    output
}

fn find_primary_symbol(origin: &WalrusFlowOpening, shard_index: u32, leaf_index: u32) -> &[u8] {
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

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_be_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}
