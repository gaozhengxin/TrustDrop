// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {HVSS, HVSSPublicValues} from "../src/HVSS.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    bytes proof;
    bytes publicValues;
    bytes32 vkey;
}

contract HVSSGroth16Test is Test {
    using stdJson for string;

    address verifier;
    HVSS public hvss;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/src/fixtures/groth16-fixture.json"
        );
        string memory json = vm.readFile(path);
        bytes memory jsonBytes = json.parseRaw(".");
        return abi.decode(jsonBytes, (SP1ProofFixtureJson));
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        hvss = new HVSS(verifier, fixture.vkey);
    }

    function test_ValidHVSSProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        HVSSPublicValues.HVSSPublicValuesStruct memory publicValues = hvss
            .verifyHVSSProof(fixture.publicValues, fixture.proof);
        assert(publicValues.hKCommitment.length == publicValues.length);
        assert(publicValues.nonce.length == publicValues.length);
        assert(publicValues.cipherBlock.length == publicValues.length);
    }

    function testRevert_InvalidHVSSProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory fakeProof = new bytes(fixture.proof.length);

        hvss.verifyHVSSProof(fixture.publicValues, fakeProof);
    }
}
