// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";

interface IOracleProxyForIntegration {
    function setController(address newController) external;
    function setWhitelist(address caller, bool allowed) external;
}

interface IOracleProxyRequest {
    function request(bytes memory cCipher, address callback) external;
}

contract OracleWorkerIntegrationClient {
    bytes public lastCipher;
    uint256 public lastStatus;
    uint256 public lastEndTime;
    bytes public lastError;

    event OracleResponse(bytes cCipher, uint256 status, uint256 endTime);
    event OracleError(bytes cCipher, bytes err);

    function requestOracle(address oracleProxy, bytes calldata cCipher) external {
        IOracleProxyRequest(oracleProxy).request(cCipher, address(this));
    }

    function onResponse(bytes memory cCipher, bytes memory response) external {
        (uint256 status, uint256 endTime) = abi.decode(
            response,
            (uint256, uint256)
        );
        lastCipher = cCipher;
        lastStatus = status;
        lastEndTime = endTime;
        emit OracleResponse(cCipher, status, endTime);
    }

    function onOracleError(bytes memory cCipher, bytes memory err) external {
        lastCipher = cCipher;
        lastError = err;
        emit OracleError(cCipher, err);
    }
}

contract DeployOracleWorkerIntegrationClient is Script {
    address internal constant DEFAULT_ORACLE_PROXY =
        0x13A59912Fe91211FB7a901974997F716f11EcFe8;
    address internal constant DEFAULT_EXCHANGE_HUB =
        0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1;

    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address oracleProxy = vm.envOr(
            "ORACLE_PROXY_ADDRESS",
            DEFAULT_ORACLE_PROXY
        );
        address exchangeHub = vm.envOr("HUB_ADDRESS", DEFAULT_EXCHANGE_HUB);
        address deployer = vm.addr(deployerKey);

        vm.startBroadcast(deployerKey);
        OracleWorkerIntegrationClient client = new OracleWorkerIntegrationClient();
        IOracleProxyForIntegration(oracleProxy).setController(deployer);
        IOracleProxyForIntegration(oracleProxy).setWhitelist(
            address(client),
            true
        );
        IOracleProxyForIntegration(oracleProxy).setController(exchangeHub);
        vm.stopBroadcast();

        console.log("OracleProxy:", oracleProxy);
        console.log("ExchangeHub restored as controller:", exchangeHub);
        console.log("IntegrationClient:", address(client));
    }
}
