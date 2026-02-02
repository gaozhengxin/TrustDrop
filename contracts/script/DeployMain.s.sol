// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {ExchangeHub} from "../src/ExchangeHub.sol";
import {ExchangeChannelImplementation} from "../src/ExchangeChannel.sol";
import {OracleProxy} from "../src/oracle/OracleProxy.sol";
import {
    WalrusFunctionsConsumer
} from "../src/oracle/FunctionsConsumer_Walrus.sol";
import {Types} from "../src/lib/Types.sol";

contract DeployMain is Script {
    function run() external {
        // --- 从环境变量读取参数 ---
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        uint256 consumerKey = vm.envUint("CONSUMER_MANAGER_KEY");

        uint64 subId = uint64(vm.envUint("CL_SUB_ID"));
        address clRouter = vm.envAddress("CL_ROUTER");

        address vssAddress = vm.envAddress("VSS_ADDRESS");
        address vddAddress = vm.envAddress("VDD_ADDRESS");

        address deployerAddr = vm.addr(deployerKey);

        // --- 第一阶段：主部署 (deployerKey) ---
        vm.startBroadcast(deployerKey);

        // 先给 address(0)，后面再 update
        OracleProxy oracleProxy = new OracleProxy(address(0), subId);

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

        // --- 第二阶段：Consumer 管理部署 (consumerKey) ---
        vm.startBroadcast(consumerKey);

        WalrusFunctionsConsumer consumer = new WalrusFunctionsConsumer(
            clRouter
        );
        // 绑定所属 Proxy
        consumer.setProxy(address(oracleProxy));

        vm.stopBroadcast();

        // --- 第三阶段：回填配置 (deployerKey) ---
        vm.startBroadcast(deployerKey);
        oracleProxy.setConfig(address(consumer), subId);
        vm.stopBroadcast();

        console.log("=== Deployment Results ===");
        console.log("ExchangeHub:", address(hub));
        console.log("OracleProxy:", address(oracleProxy));
        console.log("WalrusConsumer:", address(consumer));
        console.log("Sub ID Used:", subId);
    }
}
