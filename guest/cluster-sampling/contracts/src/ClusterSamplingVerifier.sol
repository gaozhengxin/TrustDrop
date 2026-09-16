// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// @title ClusterSamplingVerifier
/// @notice Verifies the standalone SP1 cluster-sampling proof.
contract ClusterSamplingVerifier {
    struct PublicValues {
        bytes32 originBlobId;
        bytes32 samplingSeed;
        bytes32 clusterId0;
        bytes32 clusterId1;
        bytes32 clusterId2;
        bytes32 sampleCidDigest;
    }

    ISP1Verifier public immutable verifier;
    bytes32 public immutable programVKey;

    constructor(address verifierGateway, bytes32 clusterSamplingProgramVKey) {
        require(verifierGateway != address(0), "zero verifier");
        require(clusterSamplingProgramVKey != bytes32(0), "zero vkey");
        verifier = ISP1Verifier(verifierGateway);
        programVKey = clusterSamplingProgramVKey;
    }

    function verifyClusterSamplingProof(bytes calldata proof, bytes calldata publicValues)
        external
        view
        returns (PublicValues memory values)
    {
        verifier.verifyProof(programVKey, publicValues, proof);
        values = decodePublicValues(publicValues);
    }

    function decodePublicValues(bytes calldata encoded) public pure returns (PublicValues memory values) {
        require(encoded.length == 32 * 6, "invalid public values length");
        values = abi.decode(encoded, (PublicValues));
    }
}
