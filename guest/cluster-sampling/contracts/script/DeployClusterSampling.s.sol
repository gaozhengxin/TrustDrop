// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script} from "forge-std/Script.sol";
import {ClusterSamplingVerifier} from "../src/ClusterSamplingVerifier.sol";

contract DeployClusterSampling is Script {
    function run() external returns (ClusterSamplingVerifier deployed) {
        uint256 deployerPrivateKey = vm.envUint("SELLER_KEY");
        address gateway = vm.envAddress("SP1_VERIFIER_GATEWAY");
        bytes32 programVKey = vm.envBytes32("CLUSTER_SAMPLING_PROGRAM_VKEY");

        vm.startBroadcast(deployerPrivateKey);
        deployed = new ClusterSamplingVerifier(gateway, programVKey);
        vm.stopBroadcast();
    }
}
