use alloy_sol_types::sol;

sol! {
    struct VideoSamplingPublicValues {
        bytes32 originBlobId;
        bytes32 specHash;
        bytes32 samplingSeed;
        bytes32 previewCidDigest0;
        bytes32 previewCidDigest1;
        bytes32 previewCidDigest2;
    }
}
