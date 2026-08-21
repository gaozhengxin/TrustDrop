extern crate alloc;

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use walrus_core::{
    merkle::Node,
    metadata::{BlobMetadata, SliverPairMetadata},
    BlobId, EncodingType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliverPairRoots {
    pub primary: [u8; 32],
    pub secondary: [u8; 32],
}

impl SliverPairRoots {
    pub const fn new(primary: [u8; 32], secondary: [u8; 32]) -> Self {
        Self { primary, secondary }
    }
}

/// Recomputes a Walrus blob ID from the final metadata inputs.
///
/// Walrus blob IDs commit to the Merkle root over sliver-pair metadata, the
/// encoding type, and the original unencoded length. This helper intentionally
/// starts from sliver Merkle roots rather than file bytes so VDD can verify
/// sampled blob membership without re-encoding the full asset in the zkVM.
pub fn compute_blob_id_from_sliver_pair_roots(
    roots: &[SliverPairRoots],
    encoding_type: EncodingType,
    unencoded_length: u64,
) -> BlobId {
    let metadata = BlobMetadata::new(
        encoding_type,
        unencoded_length,
        roots
            .iter()
            .map(|root| SliverPairMetadata {
                primary_hash: Node::Digest(root.primary),
                secondary_hash: Node::Digest(root.secondary),
            })
            .collect(),
    );

    BlobId::from_sliver_pair_metadata(&metadata)
}

pub fn compute_blob_id_from_sliver_pair_metadata(
    sliver_pair_metadata: Vec<SliverPairMetadata>,
    encoding_type: EncodingType,
    unencoded_length: u64,
) -> BlobId {
    let metadata = BlobMetadata::new(encoding_type, unencoded_length, sliver_pair_metadata);
    BlobId::from_sliver_pair_metadata(&metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU16;
    use walrus_core::{
        encoding::{EncodingConfig, EncodingFactory as _},
        metadata::BlobMetadataApi as _,
    };

    fn standard_metadata(data: &[u8]) -> walrus_core::metadata::VerifiedBlobMetadataWithId {
        let n_shards = NonZeroU16::new(1000).expect("n_shards must be nonzero");
        let config = EncodingConfig::new(n_shards);
        let encoding_config = config.get_for_type(EncodingType::RS2);
        encoding_config
            .compute_metadata(data)
            .expect("standard Walrus metadata computation should succeed")
    }

    fn roots_from_metadata(metadata: &BlobMetadata) -> Vec<SliverPairRoots> {
        metadata
            .hashes()
            .iter()
            .map(|pair| {
                SliverPairRoots::new(pair.primary_hash.bytes(), pair.secondary_hash.bytes())
            })
            .collect()
    }

    #[test]
    fn recomputes_standard_blob_id_from_sliver_pair_roots() {
        let data = b"trustdrop walrus blob id metadata inputs";
        let standard = standard_metadata(data);

        let recomputed = compute_blob_id_from_sliver_pair_roots(
            &roots_from_metadata(standard.metadata()),
            standard.metadata().encoding_type(),
            standard.metadata().unencoded_length(),
        );

        assert_eq!(recomputed, *standard.blob_id());
    }

    #[test]
    fn recomputes_standard_blob_id_for_larger_payload() {
        let data = vec![42u8; 1024 * 1024];
        let standard = standard_metadata(&data);

        let recomputed = compute_blob_id_from_sliver_pair_metadata(
            standard.metadata().hashes().clone(),
            standard.metadata().encoding_type(),
            standard.metadata().unencoded_length(),
        );

        assert_eq!(recomputed, *standard.blob_id());
    }

    #[test]
    fn blob_id_changes_when_a_sliver_root_changes() {
        let data = b"trustdrop walrus blob id tamper check";
        let standard = standard_metadata(data);
        let mut roots = roots_from_metadata(standard.metadata());
        roots[0].primary[0] ^= 1;

        let tampered = compute_blob_id_from_sliver_pair_roots(
            &roots,
            standard.metadata().encoding_type(),
            standard.metadata().unencoded_length(),
        );

        assert_ne!(tampered, *standard.blob_id());
    }
}
