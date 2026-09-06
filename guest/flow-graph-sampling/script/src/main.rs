use alloy_sol_types::SolValue;
use drop_lib::{
    rslh_ve::{walrus_symbol_size, COL_HEIGHT_SECONDARY},
    walrus_blob_id::SliverPairRoots,
};
use flow_graph_sampling_guest_lib::{
    AuthenticatedPrimarySliver, AuthenticatedPrimarySymbol, FlowGraphSamplingPublicValues,
    FlowSamplingWitness, MultiproofNode, WalrusFlowOpening, ARCHIVE_ENTRY_LEN, ARCHIVE_HEADER_LEN,
    ARCHIVE_MAGIC,
};
use sp1_sdk::{include_elf, Prover, ProverClient, SP1Stdin};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    num::NonZeroU16,
    path::{Path, PathBuf},
};
use trustdrop_flow_graph_sampling::{derive_time_window, TimeCoverage};
use walrus_core::{
    encoding::{EncodingConfig, EncodingFactory as _, SliverPair},
    fastcrypto::Blake2b256,
    merkle::{MerkleTree, Node},
    metadata::BlobMetadataApi as _,
    EncodingType,
};

const FLOW_SAMPLING_ELF: sp1_sdk::Elf = include_elf!("flow_graph_sampling_program");

#[derive(Debug)]
struct SourceBucket {
    bucket_start: u64,
    bucket_end: u64,
    bytes: Vec<u8>,
}

#[cfg(not(feature = "execute"))]
fn main() {
    let usage = "usage: flow-graph-sampling-client <entity-flows.ndjson> <output-dir> <seed-hex>";
    let input = PathBuf::from(env::args().nth(1).expect(usage));
    let output = PathBuf::from(env::args().nth(2).expect(usage));
    let seed = decode_seed(&env::args().nth(3).expect(usage));
    fs::create_dir_all(&output).expect("create output directory");

    let buckets = read_source_buckets(&input);
    let archive = build_archive(&buckets);
    let archive_path = output.join("flow-graph.archive");
    fs::write(&archive_path, &archive).expect("write flow archive");

    println!("encoding {} source bytes with Walrus RS2...", archive.len());
    let config =
        EncodingConfig::new(NonZeroU16::new(1000).unwrap()).get_for_type(EncodingType::RS2);
    let (slivers, metadata) = config
        .encode_with_metadata(archive)
        .expect("Walrus encoding");
    let coverage = TimeCoverage {
        first_timestamp: buckets.first().unwrap().bucket_start,
        last_timestamp: buckets.last().unwrap().bucket_end - 1,
        bucket_seconds: buckets[0].bucket_end - buckets[0].bucket_start,
    };
    let selected = derive_time_window(&seed, coverage).expect("derive flow sample");
    let selected_index = buckets
        .iter()
        .position(|bucket| bucket.bucket_start == selected.bucket_start)
        .expect("selected source bucket");
    let directory_end = ARCHIVE_HEADER_LEN + ARCHIVE_ENTRY_LEN * buckets.len() as u64;
    let selected_offset = directory_end
        + buckets[..selected_index]
            .iter()
            .map(|bucket| bucket.bytes.len() as u64)
            .sum::<u64>();
    let selected_len = buckets[selected_index].bytes.len() as u64;
    let symbol_size = walrus_symbol_size(metadata.metadata().unencoded_length()) as u64;
    let mut needed = BTreeMap::<u32, BTreeSet<u32>>::new();
    mark_range(&mut needed, 0, directory_end, symbol_size);
    mark_range(&mut needed, selected_offset, selected_len, symbol_size);
    let origin = build_opening_from_slivers(&slivers, &metadata, &config, needed);
    let authenticated_sliver_count = origin.primary_slivers.len();
    let witness = FlowSamplingWitness { origin };
    let witness_path = output.join("flow-graph-sampling-witness.bin");
    fs::write(&witness_path, bincode::serialize(&(witness, seed)).unwrap()).expect("write witness");
    fs::write(
        output.join("flow-sample.ndjson"),
        &buckets[selected_index].bytes,
    )
    .expect("write selected sample");
    println!("originBlobId: 0x{}", hex::encode(metadata.blob_id().0));
    println!(
        "bucket: [{}..{})",
        selected.bucket_start, selected.bucket_end
    );
    println!(
        "selectedPayloadSlivers: {}",
        sliver_count_for_range(selected_offset, selected_len, symbol_size)
    );
    println!("authenticatedSlivers: {authenticated_sliver_count}");
    println!("witness: {}", witness_path.display());
}

#[cfg(feature = "execute")]
#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    let witness_path = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: flow-graph-sampling-client <witness.bin>"),
    );
    let encoded = fs::read(&witness_path).expect("read witness");
    let (witness, seed): (FlowSamplingWitness, [u8; 32]) =
        bincode::deserialize(&encoded).expect("decode witness");
    let origin_blob_id = witness.origin.blob_id;
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    stdin.write(&seed);
    let client = ProverClient::builder().cpu().build().await;
    println!("executing authenticated flow-graph sampling guest...");
    let (public_values, report) = client
        .execute(FLOW_SAMPLING_ELF, stdin)
        .await
        .expect("SP1 guest execute");
    let values = FlowGraphSamplingPublicValues::abi_decode(public_values.as_slice())
        .expect("decode public values");
    assert_eq!(<[u8; 32]>::from(values.originBlobId), origin_blob_id);
    println!("originBlobId: 0x{}", hex::encode(origin_blob_id));
    println!("bucket: [{}..{})", values.bucketStart, values.bucketEnd);
    println!("sampleCidDigest: 0x{}", hex::encode(values.sampleCidDigest));
    println!("guestCycles: {}", report.total_instruction_count());
}

fn read_source_buckets(path: &Path) -> Vec<SourceBucket> {
    let source = fs::read(path).expect("read flow NDJSON");
    let mut buckets = Vec::<SourceBucket>::new();
    for line in source
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: serde_json::Value = serde_json::from_slice(line).expect("valid flow row JSON");
        let bucket_start = value["bucket_start"]
            .as_u64()
            .expect("numeric bucket_start");
        let bucket_end = value["bucket_end"].as_u64().expect("numeric bucket_end");
        assert!(bucket_end > bucket_start, "invalid flow bucket");
        if buckets.last().map(|bucket| bucket.bucket_start) != Some(bucket_start) {
            if let Some(previous) = buckets.last() {
                assert_eq!(
                    bucket_start, previous.bucket_end,
                    "source buckets must be contiguous"
                );
                assert_eq!(
                    bucket_end - bucket_start,
                    previous.bucket_end - previous.bucket_start
                );
            }
            buckets.push(SourceBucket {
                bucket_start,
                bucket_end,
                bytes: Vec::new(),
            });
        }
        let current = buckets.last_mut().unwrap();
        assert_eq!(current.bucket_end, bucket_end, "mixed bucket width");
        current.bytes.extend_from_slice(line);
        current.bytes.push(b'\n');
    }
    assert!(!buckets.is_empty(), "source has no flow rows");
    buckets
}

fn build_archive(buckets: &[SourceBucket]) -> Vec<u8> {
    let bucket_seconds = buckets[0].bucket_end - buckets[0].bucket_start;
    let directory_end = ARCHIVE_HEADER_LEN + ARCHIVE_ENTRY_LEN * buckets.len() as u64;
    let total_payload = buckets
        .iter()
        .map(|bucket| bucket.bytes.len())
        .sum::<usize>();
    let mut archive = Vec::with_capacity(directory_end as usize + total_payload);
    archive.extend_from_slice(ARCHIVE_MAGIC);
    archive.extend_from_slice(&bucket_seconds.to_be_bytes());
    archive.extend_from_slice(&buckets.first().unwrap().bucket_start.to_be_bytes());
    archive.extend_from_slice(&(buckets.last().unwrap().bucket_end - 1).to_be_bytes());
    archive.extend_from_slice(&(buckets.len() as u32).to_be_bytes());
    archive.extend_from_slice(&0u32.to_be_bytes());
    let mut offset = directory_end;
    for bucket in buckets {
        archive.extend_from_slice(&bucket.bucket_start.to_be_bytes());
        archive.extend_from_slice(&offset.to_be_bytes());
        archive.extend_from_slice(&(bucket.bytes.len() as u64).to_be_bytes());
        offset += bucket.bytes.len() as u64;
    }
    for bucket in buckets {
        archive.extend_from_slice(&bucket.bytes);
    }
    archive
}

fn mark_range(needed: &mut BTreeMap<u32, BTreeSet<u32>>, offset: u64, len: u64, symbol_size: u64) {
    let last = offset + len - 1;
    for flat_symbol in offset / symbol_size..=last / symbol_size {
        needed
            .entry((flat_symbol / COL_HEIGHT_SECONDARY as u64) as u32)
            .or_default()
            .insert((flat_symbol % COL_HEIGHT_SECONDARY as u64) as u32);
    }
}

fn sliver_count_for_range(offset: u64, len: u64, symbol_size: u64) -> u64 {
    let first = (offset / symbol_size) / COL_HEIGHT_SECONDARY as u64;
    let last = ((offset + len - 1) / symbol_size) / COL_HEIGHT_SECONDARY as u64;
    last - first + 1
}

fn build_opening_from_slivers(
    slivers: &[SliverPair],
    metadata: &walrus_core::metadata::VerifiedBlobMetadataWithId,
    config: &walrus_core::encoding::EncodingConfigEnum,
    needed: BTreeMap<u32, BTreeSet<u32>>,
) -> WalrusFlowOpening {
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
    WalrusFlowOpening {
        blob_id: metadata.blob_id().0,
        encoding_type: metadata.metadata().encoding_type().into(),
        unencoded_length: metadata.metadata().unencoded_length(),
        n_shards: n_shards as u32,
        metadata_root,
        primary_slivers,
    }
}

fn decode_seed(value: &str) -> [u8; 32] {
    hex::decode(value.strip_prefix("0x").unwrap_or(value))
        .expect("seed hex")
        .try_into()
        .expect("seed must be 32 bytes")
}
