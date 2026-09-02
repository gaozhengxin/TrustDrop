// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {SamplingChallengeVRFMock} from "../src/SamplingChallengeVRFMock.sol";

contract DeploySamplingChallengeVRFMock is Script {
    function run() external returns (SamplingChallengeVRFMock deployed) {
        vm.startBroadcast();
        deployed = new SamplingChallengeVRFMock();
        vm.stopBroadcast();
    }
}
