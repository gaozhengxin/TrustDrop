// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// @title VideoSamplingVerifier
/// @notice Verifies an SP1 proof and binds its six fixed public values to a certificate hash.
contract VideoSamplingVerifier {
    struct PublicValues {
        bytes32 originBlobId;
        bytes32 specHash;
        bytes32 samplingSeed;
        bytes32 previewCidDigest0;
        bytes32 previewCidDigest1;
        bytes32 previewCidDigest2;
    }

    ISP1Verifier public immutable verifier;
    bytes32 public immutable programVKey;

    constructor(address verifierGateway, bytes32 videoSamplingProgramVKey) {
        require(verifierGateway != address(0), "zero verifier");
        require(videoSamplingProgramVKey != bytes32(0), "zero vkey");
        verifier = ISP1Verifier(verifierGateway);
        programVKey = videoSamplingProgramVKey;
    }

    function verifyVideoSamplingProof(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 expectedCertificateHash
    ) external view returns (PublicValues memory values) {
        verifier.verifyProof(programVKey, publicValues, proof);
        values = decodePublicValues(publicValues);
        require(certificateHash(values) == expectedCertificateHash, "certificate mismatch");
    }

    function decodePublicValues(bytes calldata encoded) public pure returns (PublicValues memory values) {
        require(encoded.length == 32 * 6, "invalid public values length");
        values = abi.decode(encoded, (PublicValues));
    }

    function certificateHash(PublicValues memory values) public pure returns (bytes32) {
        return keccak256(abi.encode(values));
    }
}
