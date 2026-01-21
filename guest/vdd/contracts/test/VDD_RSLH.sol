// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {VDD_RSLH, VDD_RSLH_PublicValues} from "../src/VDD_RSLH.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct RSLHVEFixtureJson {
    bytes32 cOrigin;
    bytes32 cKey;
    bytes cCipher;
    bytes proof;
    bytes publicValues;
    bytes32 vkey;
}

contract VDD_RSLHTest is Test {
    using stdJson for string;

    address verifier;
    VDD_RSLH public vddRslhve;

    function loadFixture(
        string memory fileName
    ) public view returns (RSLHVEFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(root, "/src/fixtures/", fileName);
        string memory json = vm.readFile(path);

        RSLHVEFixtureJson memory fixture;
        fixture.cOrigin = json.readBytes32(".cOrigin");
        fixture.cKey = json.readBytes32(".cKey");
        fixture.cCipher = json.readBytes(".cCipher");
        fixture.vkey = json.readBytes32(".vkey");
        fixture.publicValues = json.readBytes(".publicValues");
        fixture.proof = json.readBytes(".proof");

        return fixture;
    }

    function setUp() public {
        RSLHVEFixtureJson memory fixture = loadFixture("vdd-walrus-rslh-groth16-fixture.json");
        // Mock Verifier Gateway
        verifier = address(new SP1VerifierGateway(address(1)));
        vddRslhve = new VDD_RSLH(verifier, fixture.vkey);
    }

    function test_ValidRSLHVEProof() public {
        RSLHVEFixtureJson memory fixture = loadFixture("vdd-walrus-rslh-groth16-fixture.json");

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        VDD_RSLH_PublicValues.VDD_RSLH_PublicValuesStruct memory pv = vddRslhve.verifyVDDProof(
            fixture.publicValues,
            fixture.proof
        );

        assertEq(pv.cOrigin, fixture.cOrigin, "cOrigin mismatch");
        assertEq(pv.cKey, fixture.cKey, "cKey mismatch");
        assertEq(keccak256(pv.cCipher), keccak256(fixture.cCipher), "cCipher mismatch");
    }

    function testRevert_InvalidRSLHVEProof() public {
        RSLHVEFixtureJson memory fixture = loadFixture("vdd-walrus-rslh-groth16-fixture.json");

        vm.expectRevert();
        bytes memory fakeProof = new bytes(fixture.proof.length);
        vddRslhve.verifyVDDProof(fixture.publicValues, fakeProof);
    }
}