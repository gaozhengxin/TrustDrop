// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {VSS, VSSPublicValues} from "../src/VSS.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    uint256 length;
    bytes32 hOrigBlock;
    bytes32[] hKCommitment;
    bytes12[] nonce;
    bytes proof;
    bytes publicValues;
    bytes32 vkey;
}

contract VSSGroth16Test is Test {
    using stdJson for string;

    address verifier;
    VSS public vss;

    function toBytes12Array(
        bytes[] memory input
    ) internal pure returns (bytes12[] memory) {
        bytes12[] memory output = new bytes12[](input.length);
        for (uint256 i = 0; i < input.length; i++) {
            bytes memory b = input[i];
            bytes32 temp;
            assembly {
                temp := mload(add(b, 32))
            }
            output[i] = bytes12(temp);
        }
        return output;
    }

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/src/fixtures/groth16-fixture.json"
        );
        string memory json = vm.readFile(path);

        SP1ProofFixtureJson memory fixture;
        fixture.length = json.readUint(".length");
        fixture.hOrigBlock = json.readBytes32(".hOrigBlock");
        fixture.hKCommitment = json.readBytes32Array(".hKCommitment");
        bytes[] memory rawNonces = json.readBytesArray(".nonce");
        fixture.nonce = toBytes12Array(rawNonces);
        fixture.vkey = json.readBytes32(".vkey");
        fixture.publicValues = json.readBytes(".publicValues");
        fixture.proof = json.readBytes(".proof");

        return fixture;
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        vss = new VSS(verifier, fixture.vkey);
    }

    function test_ValidVSSProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        // check fixture basic
        assertEq(
            fixture.hKCommitment.length,
            fixture.length,
            "hKCommitment length mismatch"
        );
        assertEq(fixture.nonce.length, fixture.length, "nonce length mismatch");

        assertTrue(fixture.hOrigBlock != bytes32(0), "hOrigBlock is zero");
        assertTrue(fixture.vkey != bytes32(0), "vkey is zero");

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        VSSPublicValues.VSSPublicValuesStruct memory publicValues = vss
            .verifyVSSProof(fixture.publicValues, fixture.proof);

        // Fixture vs PublicValues
        assertEq(publicValues.length, fixture.length, "PV: length mismatch");
        assertEq(
            publicValues.hOrigBlock,
            fixture.hOrigBlock,
            "PV: hOrigBlock mismatch"
        );

        for (uint i = 0; i < fixture.length; i++) {
            assertEq(
                publicValues.hKCommitment[i],
                fixture.hKCommitment[i],
                string.concat(
                    "PV: hKCommitment mismatch at index ",
                    vm.toString(i)
                )
            );

            assertEq(
                bytes32(publicValues.nonce[i]),
                bytes32(fixture.nonce[i]),
                "Nonce mismatch"
            );
        }
    }

    function testRevert_InvalidVSSProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory fakeProof = new bytes(fixture.proof.length);

        vss.verifyVSSProof(fixture.publicValues, fakeProof);
    }
}
