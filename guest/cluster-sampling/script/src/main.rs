#[cfg(any(feature = "execute", feature = "network"))]
use alloy_sol_types::SolValue;
#[cfg(any(feature = "execute", feature = "network"))]
use cluster_sampling_guest_lib::ClusterSamplingPublicValues;
use cluster_sampling_guest_lib::{
    AuthenticatedPrimarySliver, AuthenticatedPrimarySymbol, ClusterSamplingWitness, MultiproofNode,
    WalrusClusterOpening, ARCHIVE_ENTRY_LEN, ARCHIVE_HEADER_LEN, ARCHIVE_MAGIC, CLUSTER_ID_LEN,
    SAMPLE_CLUSTER_COUNT,
};
use drop_lib::{
    cid::compute_ipfs_cid,
    rslh_ve::{walrus_symbol_size, COL_HEIGHT_SECONDARY},
    walrus_blob_id::SliverPairRoots,
};
#[cfg(any(feature = "execute", feature = "network"))]
use sp1_sdk::{include_elf, Prover, ProverClient, SP1Stdin};
#[cfg(feature = "network")]
use sp1_sdk::{network::NetworkMode, HashableKey, ProveRequest, ProvingKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    num::NonZeroU16,
    path::{Path, PathBuf},
};
use trustdrop_cluster_sampling::derive_cluster_indices;
use walrus_core::{
    encoding::{EncodingConfig, EncodingFactory as _, SliverPair},
    fastcrypto::Blake2b256,
    merkle::{MerkleTree, Node},
    metadata::BlobMetadataApi as _,
    EncodingType,
};

#[cfg(any(feature = "execute", feature = "network"))]
const CLUSTER_SAMPLING_ELF: sp1_sdk::Elf = include_elf!("cluster_sampling_program");

#[derive(Debug)]
struct SourceCluster {
    cluster_id: [u8; CLUSTER_ID_LEN],
    bytes: Vec<u8>,
}

#[cfg(not(any(feature = "execute", feature = "network")))]
fn main() {
    let usage = "usage: cluster-sampling-client <cluster-subgraphs.ndjson> <output-dir> <seed-hex>";
    let input = PathBuf::from(env::args().nth(1).expect(usage));
    let output = PathBuf::from(env::args().nth(2).expect(usage));
    let seed = decode_seed(&env::args().nth(3).expect(usage));
    fs::create_dir_all(&output).expect("create output directory");

    let clusters = read_source_clusters(&input);
    let archive = build_archive(&clusters);
    let archive_path = output.join("cluster.archive");
    fs::write(&archive_path, &archive).expect("write cluster archive");

    println!("encoding {} source bytes with Walrus RS2...", archive.len());
    let config =
        EncodingConfig::new(NonZeroU16::new(1000).unwrap()).get_for_type(EncodingType::RS2);
    let (slivers, metadata) = config
        .encode_with_metadata(archive)
        .expect("Walrus encoding");
    let mut selected_indices = [0u64; SAMPLE_CLUSTER_COUNT];
    derive_cluster_indices(&seed, clusters.len() as u64, &mut selected_indices)
        .expect("derive cluster sample");
    let directory_end = ARCHIVE_HEADER_LEN + ARCHIVE_ENTRY_LEN * clusters.len() as u64;
    let offsets = cluster_offsets(&clusters, directory_end);
    let symbol_size = walrus_symbol_size(metadata.metadata().unencoded_length()) as u64;
    let mut needed = BTreeMap::<u32, BTreeSet<u32>>::new();
    mark_range(&mut needed, 0, directory_end, symbol_size);
    for index in selected_indices {
        let cluster = &clusters[index as usize];
        mark_range(
            &mut needed,
            offsets[index as usize],
            cluster.bytes.len() as u64,
            symbol_size,
        );
    }
    let origin = build_opening_from_slivers(&slivers, &metadata, &config, needed);
    let authenticated_sliver_count = origin.primary_slivers.len();
    let witness = ClusterSamplingWitness { origin };
    let witness_path = output.join("cluster-sampling-witness.bin");
    fs::write(&witness_path, bincode::serialize(&(witness, seed)).unwrap()).expect("write witness");
    let sample = build_sample(&clusters, &selected_indices);
    let sample_cid = compute_ipfs_cid(&sample);
    fs::write(output.join("cluster-sample.json"), sample).expect("write selected sample");
    println!("originBlobId: 0x{}", hex::encode(metadata.blob_id().0));
    println!(
        "clusterIds: {}",
        selected_cluster_ids(&clusters, &selected_indices).join(",")
    );
    println!("sampleCid: {sample_cid}");
    println!("authenticatedSlivers: {authenticated_sliver_count}");
    println!("witness: {}", witness_path.display());
}

#[cfg(feature = "network")]
#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    let witness_path = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: cluster-sampling-client <witness.bin> <proof.json>"),
    );
    let proof_path = PathBuf::from(
        env::args()
            .nth(2)
            .expect("usage: cluster-sampling-client <witness.bin> <proof.json>"),
    );
    let encoded = fs::read(&witness_path).expect("read witness");
    let (witness, seed): (ClusterSamplingWitness, [u8; 32]) =
        bincode::deserialize(&encoded).expect("decode witness");
    let origin_blob_id = witness.origin.blob_id;

    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    stdin.write(&seed);

    let private_key = network_private_key();
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .private_key(&private_key)
        .build()
        .await;
    let pk = client
        .setup(CLUSTER_SAMPLING_ELF)
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

    let values = ClusterSamplingPublicValues::abi_decode(proof.public_values.as_slice())
        .expect("decode public values");
    assert_eq!(<[u8; 32]>::from(values.originBlobId), origin_blob_id);
    assert_eq!(<[u8; 32]>::from(values.samplingSeed), seed);
    let fixture = serde_json::json!({
        "originBlobId": format!("0x{}", hex::encode(origin_blob_id)),
        "samplingSeed": format!("0x{}", hex::encode(seed)),
        "clusterIds": public_cluster_ids(&values),
        "sampleCidDigest": format!("0x{}", hex::encode(values.sampleCidDigest)),
        "programVKey": pk.verifying_key().bytes32(),
        "publicValues": format!("0x{}", hex::encode(proof.public_values.as_slice())),
        "proof": format!("0x{}", hex::encode(proof.bytes())),
    });
    fs::write(&proof_path, serde_json::to_vec_pretty(&fixture).unwrap())
        .expect("write proof fixture");
    println!("clusterIds: {}", public_cluster_ids(&values).join(","));
    println!("sampleCidDigest: 0x{}", hex::encode(values.sampleCidDigest));
    println!("networkProof: {}", proof_path.display());
}

#[cfg(feature = "execute")]
#[tokio::main]
async fn main() {
    sp1_sdk::utils::setup_logger();
    let witness_path = PathBuf::from(
        env::args()
            .nth(1)
            .expect("usage: cluster-sampling-client <witness.bin>"),
    );
    let encoded = fs::read(&witness_path).expect("read witness");
    let (witness, seed): (ClusterSamplingWitness, [u8; 32]) =
        bincode::deserialize(&encoded).expect("decode witness");
    let origin_blob_id = witness.origin.blob_id;
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    stdin.write(&seed);
    let client = ProverClient::builder().cpu().build().await;
    println!("executing authenticated cluster sampling guest...");
    let (public_values, report) = client
        .execute(CLUSTER_SAMPLING_ELF, stdin)
        .await
        .expect("SP1 guest execute");
    let values = ClusterSamplingPublicValues::abi_decode(public_values.as_slice())
        .expect("decode public values");
    assert_eq!(<[u8; 32]>::from(values.originBlobId), origin_blob_id);
    assert_eq!(<[u8; 32]>::from(values.samplingSeed), seed);
    println!("originBlobId: 0x{}", hex::encode(origin_blob_id));
    println!("clusterIds: {}", public_cluster_ids(&values).join(","));
    println!("sampleCidDigest: 0x{}", hex::encode(values.sampleCidDigest));
    println!("guestCycles: {}", report.total_instruction_count());
}

fn read_source_clusters(path: &Path) -> Vec<SourceCluster> {
    let source = fs::read(path).expect("read cluster NDJSON");
    let mut clusters = Vec::<SourceCluster>::new();
    for line in source
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let value: serde_json::Value = serde_json::from_slice(line).expect("valid cluster JSON");
        let id = value["wallet_cluster_id"]
            .as_str()
            .expect("wallet_cluster_id string");
        let cluster_id: [u8; CLUSTER_ID_LEN] = id
            .as_bytes()
            .try_into()
            .expect("wallet_cluster_id must be 28 bytes");
        assert!(
            id.starts_with("wcl_")
                && id[4..]
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        );
        clusters.push(SourceCluster {
            cluster_id,
            bytes: line.to_vec(),
        });
    }
    clusters.sort_by_key(|cluster| cluster.cluster_id);
    assert!(
        clusters.len() >= SAMPLE_CLUSTER_COUNT,
        "source has fewer than three clusters"
    );
    assert!(clusters
        .windows(2)
        .all(|pair| pair[0].cluster_id != pair[1].cluster_id));
    clusters
}

fn build_archive(clusters: &[SourceCluster]) -> Vec<u8> {
    let directory_end = ARCHIVE_HEADER_LEN + ARCHIVE_ENTRY_LEN * clusters.len() as u64;
    let total_payload = clusters
        .iter()
        .map(|cluster| cluster.bytes.len())
        .sum::<usize>();
    let mut archive = Vec::with_capacity(directory_end as usize + total_payload);
    archive.extend_from_slice(ARCHIVE_MAGIC);
    archive.extend_from_slice(&(clusters.len() as u64).to_be_bytes());
    archive.extend_from_slice(&0u64.to_be_bytes());
    let mut offset = directory_end;
    for cluster in clusters {
        archive.extend_from_slice(&cluster.cluster_id);
        archive.extend_from_slice(&0u32.to_be_bytes());
        archive.extend_from_slice(&offset.to_be_bytes());
        archive.extend_from_slice(&(cluster.bytes.len() as u64).to_be_bytes());
        offset += cluster.bytes.len() as u64;
    }
    for cluster in clusters {
        archive.extend_from_slice(&cluster.bytes);
    }
    archive
}

fn cluster_offsets(clusters: &[SourceCluster], directory_end: u64) -> Vec<u64> {
    let mut offset = directory_end;
    clusters
        .iter()
        .map(|cluster| {
            let current = offset;
            offset += cluster.bytes.len() as u64;
            current
        })
        .collect()
}

fn build_sample(clusters: &[SourceCluster], selected: &[u64; SAMPLE_CLUSTER_COUNT]) -> Vec<u8> {
    let mut sample = vec![b'['];
    for (ordinal, index) in selected.iter().enumerate() {
        if ordinal > 0 {
            sample.push(b',');
        }
        sample.extend_from_slice(&clusters[*index as usize].bytes);
    }
    sample.extend_from_slice(b"]\n");
    sample
}

fn selected_cluster_ids(
    clusters: &[SourceCluster],
    selected: &[u64; SAMPLE_CLUSTER_COUNT],
) -> Vec<String> {
    selected
        .iter()
        .map(|index| String::from_utf8(clusters[*index as usize].cluster_id.to_vec()).unwrap())
        .collect()
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

fn build_opening_from_slivers(
    slivers: &[SliverPair],
    metadata: &walrus_core::metadata::VerifiedBlobMetadataWithId,
    config: &walrus_core::encoding::EncodingConfigEnum,
    needed: BTreeMap<u32, BTreeSet<u32>>,
) -> WalrusClusterOpening {
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
            let mut active = leaves.clone();
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
            let mut width = n_shards;
            let mut level = 0u8;
            while width > 1 {
                for active_index in &active {
                    proof_nodes.remove(&(level, *active_index));
                }
                active = active
                    .into_iter()
                    .map(|active_index| active_index / 2)
                    .collect();
                width = width.next_multiple_of(2) / 2;
                level += 1;
            }
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
    WalrusClusterOpening {
        blob_id: metadata.blob_id().0,
        encoding_type: metadata.metadata().encoding_type().into(),
        unencoded_length: metadata.metadata().unencoded_length(),
        n_shards: n_shards as u32,
        metadata_root,
        primary_slivers,
    }
}

#[cfg(any(feature = "execute", feature = "network"))]
fn public_cluster_ids(values: &ClusterSamplingPublicValues) -> [String; SAMPLE_CLUSTER_COUNT] {
    [
        decode_public_cluster_id(values.clusterId0.into()),
        decode_public_cluster_id(values.clusterId1.into()),
        decode_public_cluster_id(values.clusterId2.into()),
    ]
}

#[cfg(any(feature = "execute", feature = "network"))]
fn decode_public_cluster_id(value: [u8; 32]) -> String {
    assert_eq!(&value[CLUSTER_ID_LEN..], &[0u8; 32 - CLUSTER_ID_LEN]);
    String::from_utf8(value[..CLUSTER_ID_LEN].to_vec()).expect("public cluster ID is UTF-8")
}

fn decode_seed(value: &str) -> [u8; 32] {
    hex::decode(value.strip_prefix("0x").unwrap_or(value))
        .expect("seed hex")
        .try_into()
        .expect("seed must be 32 bytes")
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

#[cfg(all(feature = "execute", feature = "network"))]
compile_error!("features execute and network are mutually exclusive");
