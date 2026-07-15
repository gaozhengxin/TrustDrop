// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {VisionRegistry} from "../src/VisionRegistry.sol";

contract DeployVisionRegistry is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        string memory initialVisionCid = vm.envOr("VISION_CID", string(""));
        address deployer = vm.addr(deployerKey);

        vm.startBroadcast(deployerKey);
        VisionRegistry registry = new VisionRegistry(deployer, initialVisionCid);
        vm.stopBroadcast();

        console.log("VisionRegistry:", address(registry));
        console.log("VisionAdmin:", deployer);
        console.log("VisionCID:", initialVisionCid);
    }
}
