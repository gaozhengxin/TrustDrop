// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {VDD_RSLH} from "../src/VDD_RSLH.sol";

contract DeployVDD is Script {
    using stdJson for string;

    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        // 使用 Arbitrum Sepolia 的 SP1 Verifier Gateway 地址
        address verifierGateway = vm.envAddress("SP1_VERIFIER_GATEWAY");

        // 1. 读取 Fixture 获取 VKey
        string memory root = vm.projectRoot();
        string memory path = string.concat(root, "/src/fixtures/vdd-walrus-rslh-groth16-fixture.json");
        string memory json = vm.readFile(path);
        bytes32 vkey = json.readBytes32(".vkey");

        console.log("Extracted VDD VKey:", vm.toString(vkey));

        // 2. 开始部署
        vm.startBroadcast(deployerPrivateKey);

        VDD_RSLH vddContract = new VDD_RSLH(verifierGateway, vkey);

        vm.stopBroadcast();

        console.log("-----------------------------------------");
        console.log("VDD_RSLH deployed successfully!");
        console.log("Address:", address(vddContract));
        console.log("-----------------------------------------");
    }
}