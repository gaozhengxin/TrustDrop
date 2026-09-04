// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {SamplingChallengeVRFMock} from "../src/SamplingChallengeVRFMock.sol";

contract SamplingChallengeVRFMockTest is Test {
    SamplingChallengeVRFMock internal vrf;
    bytes32 internal constant KEY = keccak256("sale/video");

    function setUp() public {
        vrf = new SamplingChallengeVRFMock();
    }

    function testRequestSynchronouslyStoresSeed() public {
        (bytes32 requestId, bytes32 seed) = vrf.requestSeed(KEY);
        (address requester, bytes32 storedSeed, bytes32 storedRequestId, uint64 count, uint64 blockNumber) =
            vrf.latestChallenges(address(this), KEY);
        assertEq(requester, address(this));
        assertEq(storedSeed, seed);
        assertEq(storedRequestId, requestId);
        assertEq(count, 1);
        assertEq(blockNumber, block.number);
    }

    function testRepeatedRequestReplacesLatestRecord() public {
        (bytes32 firstRequestId, bytes32 firstSeed) = vrf.requestSeed(KEY);
        (bytes32 secondRequestId, bytes32 secondSeed) = vrf.requestSeed(KEY);
        (, bytes32 storedSeed, bytes32 storedRequestId, uint64 count,) = vrf.latestChallenges(address(this), KEY);
        assertNotEq(secondRequestId, firstRequestId);
        assertNotEq(secondSeed, firstSeed);
        assertEq(storedSeed, secondSeed);
        assertEq(storedRequestId, secondRequestId);
        assertEq(count, 2);
    }

    function testDifferentRequestersCannotOverwriteSellerLatest() public {
        (, bytes32 sellerSeed) = vrf.requestSeed(KEY);
        vm.prank(address(0xBEEF));
        vrf.requestSeed(KEY);
        (address requester,,, uint64 count,) = vrf.latestChallenges(address(0xBEEF), KEY);
        assertEq(requester, address(0xBEEF));
        assertEq(count, 1);
        (, bytes32 storedSellerSeed,, uint64 sellerCount,) = vrf.latestChallenges(address(this), KEY);
        assertEq(storedSellerSeed, sellerSeed);
        assertEq(sellerCount, 1);
    }

    function testCallbackCannotBeCalledExternally() public {
        vm.expectRevert("only self callback");
        vrf.fulfillSeed(KEY, address(this), bytes32(uint256(1)), bytes32(uint256(2)), 1);
    }
}
