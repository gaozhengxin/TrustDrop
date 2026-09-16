// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// @title FlowGraphSamplingVerifier
/// @notice Verifies the standalone SP1 flow-graph sampling proof.
contract FlowGraphSamplingVerifier {
    struct PublicValues {
        bytes32 originBlobId;
        bytes32 samplingSeed;
        uint64 bucketStart;
        uint64 bucketEnd;
        bytes32 sampleCidDigest;
    }

    ISP1Verifier public immutable verifier;
    bytes32 public immutable programVKey;

    constructor(address verifierGateway, bytes32 flowSamplingProgramVKey) {
        require(verifierGateway != address(0), "zero verifier");
        require(flowSamplingProgramVKey != bytes32(0), "zero vkey");
        verifier = ISP1Verifier(verifierGateway);
        programVKey = flowSamplingProgramVKey;
    }

    function verifyFlowGraphSamplingProof(bytes calldata proof, bytes calldata publicValues)
        external
        view
        returns (PublicValues memory values)
    {
        verifier.verifyProof(programVKey, publicValues, proof);
        values = decodePublicValues(publicValues);
    }

    function decodePublicValues(bytes calldata encoded) public pure returns (PublicValues memory values) {
        require(encoded.length == 32 * 5, "invalid public values length");
        values = abi.decode(encoded, (PublicValues));
    }
}
