use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};
use walrus_core::merkle::Node;

pub const ARCHIVE_MAGIC: &[u8; 8] = b"TDFLOW01";
pub const ARCHIVE_HEADER_LEN: u64 = 40;
pub const ARCHIVE_ENTRY_LEN: u64 = 24;

sol! {
    struct FlowGraphSamplingPublicValues {
        bytes32 originBlobId;
        bytes32 samplingSeed;
        uint64 bucketStart;
        uint64 bucketEnd;
        bytes32 sampleCidDigest;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSamplingWitness {
    pub origin: WalrusFlowOpening,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalrusFlowOpening {
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
