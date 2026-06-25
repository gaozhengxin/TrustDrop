// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";

interface IOracleProxyAdmin {
    function setCentralizedOracleSigner(address signer) external;
}

contract SetCentralizedOracleSigner is Script {
    address internal constant DEFAULT_ORACLE_PROXY =
        0x13A59912Fe91211FB7a901974997F716f11EcFe8;

    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        uint256 oracleRelayerKey = vm.envUint("ORACLE_RELAYER_PRIVATE_KEY");
        address oracleProxy = vm.envOr(
            "ORACLE_PROXY_ADDRESS",
            DEFAULT_ORACLE_PROXY
        );
        address oracleSigner = vm.addr(oracleRelayerKey);

        vm.startBroadcast(deployerKey);
        IOracleProxyAdmin(oracleProxy).setCentralizedOracleSigner(oracleSigner);
        vm.stopBroadcast();

        console.log("OracleProxy:", oracleProxy);
        console.log("CentralizedOracleSigner:", oracleSigner);
    }
}
