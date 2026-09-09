#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolValue;
use cluster_sampling_guest_lib::{
    AuthenticatedPrimarySliver, ClusterSamplingPublicValues, ClusterSamplingWitness,
    WalrusClusterOpening, ARCHIVE_ENTRY_LEN, ARCHIVE_HEADER_LEN, ARCHIVE_MAGIC, CLUSTER_ID_LEN,
    SAMPLE_CLUSTER_COUNT,
};
use drop_lib::{
    cid::compute_ipfs_cid_zk_optimized,
    rslh_ve::{walrus_symbol_size, COL_HEIGHT_SECONDARY},
};
use std::collections::{BTreeMap, BTreeSet};
use trustdrop_cluster_sampling::derive_cluster_indices;
use walrus_core::{
    fastcrypto::Blake2b256,
    merkle::{MerkleAuth, MerkleProof, MerkleTree, Node},
    BlobId, EncodingType,
};

#[derive(Debug, Clone, Copy)]
struct ClusterEntry {
    cluster_id: [u8; CLUSTER_ID_LEN],
    offset: u64,
    len: u64,
}

pub fn main() {
    let witness = sp1_zkvm::io::read::<ClusterSamplingWitness>();
    let sampling_seed = sp1_zkvm::io::read::<[u8; 32]>();

    verify_walrus_opening(&witness.origin);
    let (entries, directory_end) = read_authenticated_directory(&witness.origin);
    let mut selected_indices = [0u64; SAMPLE_CLUSTER_COUNT];
    derive_cluster_indices(&sampling_seed, entries.len() as u64, &mut selected_indices)
        .expect("invalid cluster catalog");

    let symbol_size = walrus_symbol_size(witness.origin.unencoded_length) as u64;
    let mut required = BTreeMap::<u32, BTreeSet<u32>>::new();
    mark_range(&mut required, 0, directory_end, symbol_size);
    for index in selected_indices {
        let entry = entries[index as usize];
        mark_range(&mut required, entry.offset, entry.len, symbol_size);
    }
    assert_exact_symbol_selection(&witness.origin, &required);

    let mut sample = Vec::new();
    sample.push(b'[');
    for (ordinal, index) in selected_indices.into_iter().enumerate() {
        if ordinal > 0 {
            sample.push(b',');
        }
        let entry = entries[index as usize];
        let record = read_origin_range(&witness.origin, entry.offset, entry.len);
        validate_cluster_record(&record, &entry.cluster_id);
        sample.extend_from_slice(&record);
    }
    sample.extend_from_slice(b"]\n");
    let cid = compute_ipfs_cid_zk_optimized(&sample);
    let mut sample_cid_digest = [0u8; 32];
    sample_cid_digest.copy_from_slice(&cid[cid.len() - 32..]);

    let cluster_ids =
        selected_indices.map(|index| public_cluster_id(entries[index as usize].cluster_id));
    let values = ClusterSamplingPublicValues {
        originBlobId: witness.origin.blob_id.into(),
        samplingSeed: sampling_seed.into(),
        clusterId0: cluster_ids[0].into(),
        clusterId1: cluster_ids[1].into(),
        clusterId2: cluster_ids[2].into(),
        sampleCidDigest: sample_cid_digest.into(),
    };
    sp1_zkvm::io::commit_slice(&values.abi_encode());
}

fn read_authenticated_directory(origin: &WalrusClusterOpening) -> (Vec<ClusterEntry>, u64) {
    let header = read_origin_range(origin, 0, ARCHIVE_HEADER_LEN);
    assert_eq!(&header[..8], ARCHIVE_MAGIC, "invalid cluster archive magic");
    let entry_count = read_u64(&header, 8);
    assert_eq!(
        read_u64(&header, 16),
        0,
        "cluster archive reserved field changed"
    );
    assert!(
        entry_count >= SAMPLE_CLUSTER_COUNT as u64,
        "cluster catalog is too small"
    );
    let directory_len = entry_count
        .checked_mul(ARCHIVE_ENTRY_LEN)
        .expect("cluster directory length overflow");
    let directory_end = ARCHIVE_HEADER_LEN
        .checked_add(directory_len)
        .expect("cluster directory end overflow");
    let bytes = read_origin_range(origin, ARCHIVE_HEADER_LEN, directory_len);
    let mut entries = Vec::<ClusterEntry>::with_capacity(entry_count as usize);
    let mut expected_offset = directory_end;
    for chunk in bytes.chunks_exact(ARCHIVE_ENTRY_LEN as usize) {
        let mut cluster_id = [0u8; CLUSTER_ID_LEN];
        cluster_id.copy_from_slice(&chunk[..CLUSTER_ID_LEN]);
        assert_eq!(
            read_u32(chunk, 28),
            0,
            "cluster entry reserved field changed"
        );
        validate_cluster_id(&cluster_id);
        let entry = ClusterEntry {
            cluster_id,
            offset: read_u64(chunk, 32),
            len: read_u64(chunk, 40),
        };
        assert_eq!(
            entry.offset, expected_offset,
            "non-contiguous cluster payload"
        );
        assert!(entry.len > 0, "empty cluster payload");
        expected_offset = expected_offset
            .checked_add(entry.len)
            .expect("cluster payload length overflow");
        if let Some(previous) = entries.last() {
            assert!(
                previous.cluster_id < entry.cluster_id,
                "cluster IDs not canonical"
            );
        }
        entries.push(entry);
    }
    assert_eq!(
        expected_offset, origin.unencoded_length,
        "archive length mismatch"
    );
    (entries, directory_end)
}

fn validate_cluster_id(cluster_id: &[u8; CLUSTER_ID_LEN]) {
    assert_eq!(&cluster_id[..4], b"wcl_", "invalid cluster ID prefix");
    assert!(
        cluster_id[4..]
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "invalid cluster ID"
    );
}

fn validate_cluster_record(record: &[u8], expected_cluster_id: &[u8; CLUSTER_ID_LEN]) {
    assert_eq!(
        record.first(),
        Some(&b'{'),
        "cluster record must be a JSON object"
    );
    assert_eq!(
        record.last(),
        Some(&b'}'),
        "cluster record must end at object boundary"
    );
    const FIELD: &[u8] = b"\"wallet_cluster_id\":\"";
    let field_offset = record
        .windows(FIELD.len())
        .position(|window| window == FIELD)
        .expect("cluster record missing wallet_cluster_id");
    let value_offset = field_offset + FIELD.len();
    let value_end = value_offset + CLUSTER_ID_LEN;
    assert!(value_end < record.len(), "truncated wallet_cluster_id");
    assert_eq!(
        &record[value_offset..value_end],
        expected_cluster_id,
        "cluster directory ID does not match record"
    );
    assert_eq!(
        record[value_end], b'"',
        "wallet_cluster_id has wrong length"
    );
    assert!(
        record[value_end + 1..]
            .windows(FIELD.len())
            .all(|window| window != FIELD),
        "duplicate wallet_cluster_id field"
    );
}

fn public_cluster_id(cluster_id: [u8; CLUSTER_ID_LEN]) -> [u8; 32] {
    let mut output = [0u8; 32];
    output[..CLUSTER_ID_LEN].copy_from_slice(&cluster_id);
    output
}

fn assert_exact_symbol_selection(
    origin: &WalrusClusterOpening,
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

fn verify_walrus_opening(origin: &WalrusClusterOpening) {
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

fn read_origin_range(origin: &WalrusClusterOpening, offset: u64, len: u64) -> Vec<u8> {
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

fn find_primary_symbol(origin: &WalrusClusterOpening, shard_index: u32, leaf_index: u32) -> &[u8] {
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
