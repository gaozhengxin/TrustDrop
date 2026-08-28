// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {VideoSamplingVerifier} from "../src/VideoSamplingVerifier.sol";

contract MockSP1Verifier is ISP1Verifier {
    bytes32 public expectedVKey;

    constructor(bytes32 vkey) {
        expectedVKey = vkey;
    }

    function verifyProof(bytes32 vkey, bytes calldata, bytes calldata proof) external view {
        require(vkey == expectedVKey, "wrong vkey");
        require(keccak256(proof) == keccak256("valid-proof"), "invalid proof");
    }
}

contract VideoSamplingVerifierTest is Test {
    bytes32 internal constant VKEY = bytes32(uint256(1));
    VideoSamplingVerifier internal verifier;

    function setUp() public {
        verifier = new VideoSamplingVerifier(address(new MockSP1Verifier(VKEY)), VKEY);
    }

    function testVerifiesProofAndCertificateBinding() public view {
        VideoSamplingVerifier.PublicValues memory values = sampleValues();
        bytes memory encoded = abi.encode(values);
        bytes32 binding = verifier.certificateHash(values);
        VideoSamplingVerifier.PublicValues memory decoded =
            verifier.verifyVideoSamplingProof(bytes("valid-proof"), encoded, binding);
        assertEq(decoded.sourceCommitment, values.sourceCommitment);
        assertEq(decoded.evidenceCidDigest2, values.evidenceCidDigest2);
    }

    function testRejectsWrongCertificateBinding() public {
        bytes memory encoded = abi.encode(sampleValues());
        vm.expectRevert("certificate mismatch");
        verifier.verifyVideoSamplingProof(bytes("valid-proof"), encoded, bytes32(uint256(9)));
    }

    function sampleValues() private pure returns (VideoSamplingVerifier.PublicValues memory values) {
        values = VideoSamplingVerifier.PublicValues({
            sourceCommitment: bytes32(uint256(1)),
            specHash: bytes32(uint256(2)),
            samplingSeed: bytes32(uint256(3)),
            evidenceCidDigest0: bytes32(uint256(4)),
            evidenceCidDigest1: bytes32(uint256(5)),
            evidenceCidDigest2: bytes32(uint256(6))
        });
    }
}
