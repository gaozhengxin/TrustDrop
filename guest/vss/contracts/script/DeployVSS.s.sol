// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {VSS} from "../src/VSS.sol";

contract DeployVSS is Script {
    using stdJson for string;

    function run() external {
        // 1. 加载 Fixture 获取 vkey
        string memory root = vm.projectRoot();
        string memory path = string.concat(root, "/src/fixtures/groth16-fixture.json");
        string memory json = vm.readFile(path);
        bytes32 vkey = json.readBytes32(".vkey");

        // 2. 配置环境
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        // Arbitrum Sepolia SP1 Gateway
        address sp1VerifierGateway = vm.envAddress("SP1_VERIFIER_GATEWAY");

        vm.startBroadcast(deployerPrivateKey);

        VSS vss = new VSS(sp1VerifierGateway, vkey);
        
        console.log("VSS deployed at:", address(vss));
        console.log("VKey extracted from fixture:", vm.toString(vkey));

        vm.stopBroadcast();
    }
}