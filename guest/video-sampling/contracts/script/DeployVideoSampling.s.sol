// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {VideoSamplingVerifier} from "../src/VideoSamplingVerifier.sol";

contract DeployVideoSampling is Script {
    function run() external returns (VideoSamplingVerifier deployed) {
        address gateway = vm.envAddress("SP1_VERIFIER_GATEWAY");
        bytes32 programVKey = vm.envBytes32("VIDEO_SAMPLING_PROGRAM_VKEY");

        vm.startBroadcast();
        deployed = new VideoSamplingVerifier(gateway, programVKey);
        vm.stopBroadcast();
    }
}
