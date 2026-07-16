// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {VisionRegistry} from "../src/VisionRegistry.sol";

contract SetVisionCid is Script {
    function run() external {
        uint256 adminKey = vm.envUint("PRIVATE_KEY");
        address registry = vm.envAddress("VISION_REGISTRY_ADDRESS");
        string memory cid = vm.envString("VISION_CID");

        vm.startBroadcast(adminKey);
        VisionRegistry(registry).setVisionCid(cid);
        vm.stopBroadcast();

        console.log("VisionRegistry:", registry);
        console.log("VisionCID:", cid);
    }
}
