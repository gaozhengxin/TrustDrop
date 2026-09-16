// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {FlowGraphSamplingVerifier} from "../src/FlowGraphSamplingVerifier.sol";

contract DeployFlowGraphSampling is Script {
    function run() external returns (FlowGraphSamplingVerifier deployed) {
        uint256 deployerPrivateKey = vm.envUint("SELLER_KEY");
        address gateway = vm.envAddress("SP1_VERIFIER_GATEWAY");
        bytes32 programVKey = vm.envBytes32("FLOW_SAMPLING_PROGRAM_VKEY");

        vm.startBroadcast(deployerPrivateKey);
        deployed = new FlowGraphSamplingVerifier(gateway, programVKey);
        vm.stopBroadcast();
    }
}
