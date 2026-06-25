// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {ExchangeHub} from "../src/ExchangeHub.sol";
import {ExchangeChannelImplementation} from "../src/ExchangeChannel.sol";
import {OracleProxy} from "../src/oracle/OracleProxy.sol";
import {Types} from "../src/lib/Types.sol";

contract DeployMain is Script {
    address internal constant ARBITRUM_SEPOLIA_CRE_FORWARDER =
        0x76c9cf548b4179F8901cda1f8623568b58215E62;

    function run() external {
        // --- 从环境变量读取参数 ---
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");

        address centralizedOracleSigner = vm.envOr(
            "CENTRALIZED_ORACLE_SIGNER",
            address(0)
        );
        address creForwarder = vm.envOr(
            "CRE_FORWARDER",
            ARBITRUM_SEPOLIA_CRE_FORWARDER
        );

        address vssAddress = vm.envAddress("VSS_ADDRESS");
        address vddAddress = vm.envAddress("VDD_ADDRESS");

        address deployerAddr = vm.addr(deployerKey);

        // --- 第一阶段：主部署 (deployerKey) ---
        vm.startBroadcast(deployerKey);

        OracleProxy oracleProxy = new OracleProxy(
            centralizedOracleSigner,
            creForwarder
        );

        ExchangeChannelImplementation implementation = new ExchangeChannelImplementation(
                Types.Pubkey(hex"00"),
                address(0),
                address(0),
                deployerAddr,
                address(0),
                address(0)
            );

        ExchangeHub hub = new ExchangeHub(
            address(oracleProxy),
            vssAddress,
            vddAddress,
            address(implementation)
        );

        oracleProxy.setController(address(hub));

        vm.stopBroadcast();

        console.log("=== Deployment Results ===");
        console.log("ExchangeHub:", address(hub));
        console.log("OracleProxy:", address(oracleProxy));
        console.log("ExchangeLogic:", address(implementation));
        console.log("CentralizedOracleSigner:", centralizedOracleSigner);
        console.log("CREForwarder:", creForwarder);
    }
}
