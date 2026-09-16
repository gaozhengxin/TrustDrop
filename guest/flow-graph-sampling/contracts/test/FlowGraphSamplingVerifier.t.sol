// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {FlowGraphSamplingVerifier} from "../src/FlowGraphSamplingVerifier.sol";

contract MockFlowSP1Verifier is ISP1Verifier {
    bytes32 public immutable expectedVKey;

    constructor(bytes32 vkey) {
        expectedVKey = vkey;
    }

    function verifyProof(bytes32 vkey, bytes calldata, bytes calldata proof) external view {
        require(vkey == expectedVKey, "wrong vkey");
        require(keccak256(proof) == keccak256("valid-proof"), "invalid proof");
    }
}

contract FlowGraphSamplingVerifierTest is Test {
    bytes32 internal constant VKEY = bytes32(uint256(1));
    FlowGraphSamplingVerifier internal verifier;

    function setUp() public {
        verifier = new FlowGraphSamplingVerifier(address(new MockFlowSP1Verifier(VKEY)), VKEY);
    }

    function testVerifiesAndDecodesPublicValues() public view {
        FlowGraphSamplingVerifier.PublicValues memory values = sampleValues();
        FlowGraphSamplingVerifier.PublicValues memory decoded =
            verifier.verifyFlowGraphSamplingProof(bytes("valid-proof"), abi.encode(values));

        assertEq(decoded.originBlobId, values.originBlobId);
        assertEq(decoded.samplingSeed, values.samplingSeed);
        assertEq(decoded.bucketStart, values.bucketStart);
        assertEq(decoded.bucketEnd, values.bucketEnd);
        assertEq(decoded.sampleCidDigest, values.sampleCidDigest);
    }

    function testRejectsWrongPublicValuesLength() public {
        vm.expectRevert("invalid public values length");
        verifier.verifyFlowGraphSamplingProof(bytes("valid-proof"), new bytes(32 * 4));
    }

    function testRejectsWrongProgramProof() public {
        vm.expectRevert("invalid proof");
        verifier.verifyFlowGraphSamplingProof(bytes("wrong-proof"), abi.encode(sampleValues()));
    }

    function testActualSuccinctProofWhenConfigured() public {
        string memory fixturePath = vm.envOr("FLOW_PROOF_FIXTURE", string(""));
        if (bytes(fixturePath).length == 0) return;

        string memory fixture = vm.readFile(fixturePath);
        bytes memory proof = vm.parseJsonBytes(fixture, ".proof");
        bytes memory publicValues = vm.parseJsonBytes(fixture, ".publicValues");
        bytes32 vkey = vm.parseJsonBytes32(fixture, ".programVKey");
        address gateway = vm.envAddress("SP1_VERIFIER_GATEWAY");
        FlowGraphSamplingVerifier liveVerifier = new FlowGraphSamplingVerifier(gateway, vkey);

        FlowGraphSamplingVerifier.PublicValues memory decoded =
            liveVerifier.verifyFlowGraphSamplingProof(proof, publicValues);

        assertEq(decoded.originBlobId, vm.parseJsonBytes32(fixture, ".originBlobId"));
        assertEq(decoded.samplingSeed, vm.parseJsonBytes32(fixture, ".samplingSeed"));
        assertEq(decoded.sampleCidDigest, vm.parseJsonBytes32(fixture, ".sampleCidDigest"));
    }

    function sampleValues() private pure returns (FlowGraphSamplingVerifier.PublicValues memory values) {
        values = FlowGraphSamplingVerifier.PublicValues({
            originBlobId: bytes32(uint256(1)),
            samplingSeed: bytes32(uint256(2)),
            bucketStart: 100,
            bucketEnd: 160,
            sampleCidDigest: bytes32(uint256(3))
        });
    }
}
