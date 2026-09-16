use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};
use walrus_core::merkle::Node;

pub const SAMPLE_CLUSTER_COUNT: usize = 3;
pub const CLUSTER_ID_LEN: usize = 28;
pub const ARCHIVE_MAGIC: &[u8; 8] = b"TDCLUS01";
pub const ARCHIVE_HEADER_LEN: u64 = 24;
pub const ARCHIVE_ENTRY_LEN: u64 = 48;

sol! {
    struct ClusterSamplingPublicValues {
        bytes32 originBlobId;
        bytes32 samplingSeed;
        bytes32 clusterId0;
        bytes32 clusterId1;
        bytes32 clusterId2;
        bytes32 sampleCidDigest;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSamplingWitness {
    pub origin: WalrusClusterOpening,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalrusClusterOpening {
    pub blob_id: [u8; 32],
    pub encoding_type: u8,
    pub unencoded_length: u64,
    pub n_shards: u32,
    pub metadata_root: [u8; 32],
    pub primary_slivers: Vec<AuthenticatedPrimarySliver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedPrimarySliver {
    pub shard_index: u32,
    pub primary_root: [u8; 32],
    pub secondary_root: [u8; 32],
    pub pair_leaf_path: Vec<Node>,
    pub symbols: Vec<AuthenticatedPrimarySymbol>,
    pub proof_nodes: Vec<MultiproofNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedPrimarySymbol {
    pub leaf_index: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiproofNode {
    pub level: u8,
    pub index: u32,
    pub node: Node,
}
