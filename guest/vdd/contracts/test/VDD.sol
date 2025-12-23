// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {VDD} from "../src/VDD.sol";
import {VDDPublicValues} from "../src/VDDPublicValues.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    bytes32 cOrigin;
    bytes32 cKey;
    bytes32 cCipher;
    uint32 dataLength;
    bytes proof;
    bytes publicValues;
    bytes32 vkey;
}

contract VDDTest is Test {
    using stdJson for string;

    address verifier;
    VDD public vdd;

    function loadFixture(
        string memory fileName
    ) public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(root, "/src/fixtures/", fileName);
        string memory json = vm.readFile(path);

        SP1ProofFixtureJson memory fixture;
        fixture.cOrigin = json.readBytes32(".cOrigin");
        fixture.cKey = json.readBytes32(".cKey");
        fixture.cCipher = json.readBytes32(".cCipher");

        fixture.dataLength = uint32(json.readUint(".dataLength"));

        fixture.vkey = json.readBytes32(".vkey");
        fixture.publicValues = json.readBytes(".publicValues");
        fixture.proof = json.readBytes(".proof");

        return fixture;
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture(
            "groth16-fixture.json"
        );
        verifier = address(new SP1VerifierGateway(address(1)));
        vdd = new VDD(verifier, fixture.vkey);
    }

    function test_ValidVDDProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture(
            "groth16-fixture.json"
        );

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        VDDPublicValues.VDDPublicValuesStruct memory pv = vdd.verifyVDDProof(
            fixture.publicValues,
            fixture.proof
        );

        assertEq(pv.cOrigin, fixture.cOrigin, "cOrigin mismatch");
        assertEq(pv.cKey, fixture.cKey, "cKey mismatch");
        assertEq(pv.cCipher, fixture.cCipher, "cCipher mismatch");
        assertEq(pv.dataLength, fixture.dataLength, "dataLength mismatch");
    }

    function testRevert_InvalidVDDProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture(
            "groth16-fixture.json"
        );

        vm.expectRevert();
        bytes memory fakeProof = new bytes(fixture.proof.length);
        vdd.verifyVDDProof(fixture.publicValues, fakeProof);
    }
}
