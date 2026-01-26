// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {FunctionsClient} from "@chainlink/contracts@1.5.0/src/v0.8/functions/v1_0_0/FunctionsClient.sol";
import {FunctionsRequest} from "@chainlink/contracts@1.5.0/src/v0.8/functions/v1_0_0/libraries/FunctionsRequest.sol";
import {ConfirmedOwner} from "@chainlink/contracts@1.5.0/src/v0.8/shared/access/ConfirmedOwner.sol";

interface IProxyCallback {
    function handleResponse(bytes32 requestId, bytes memory response, bytes memory err) external;
}

contract WalrusFunctionsConsumer is FunctionsClient, ConfirmedOwner {
    using FunctionsRequest for FunctionsRequest.Request;

    bytes32 public donID = 0x66756e2d617262697472756d2d7365706f6c69612d3100000000000000000000;
    uint32 public gasLimit = 300000;
    address public proxy;
    string public apiKey = "eNx0cS4PemfQtVaArXbRbHcyJTnP0l"; // 默认 Key

    // JavaScript Source - 从 args[1] 获取 API Key
    string source = 
    "const initDate = new Date('2025-12-16T00:00:00Z');"
    "const initEpoch = 20;"
    "const epochLength = 1209600;"
    "const encodeResponse = (s, t) => {"
    "const res = new Uint8Array(64);"
    "const v = new DataView(res.buffer);"
    "v.setUint32(28, s);"
    "v.setUint32(60, t);"
    "return res;"
    "};"
    "const blobIdHex = args[0].startsWith('0x') ? args[0].slice(2) : args[0];"
    "const apiKey = args[1];"
    "const base64 = Buffer.from(blobIdHex, 'hex').toString('base64').replace(/\\+/g, '-').replace(/\\//g, '_').replace(/=/g, '');"
    "const startTimeSec = initDate.getTime() / 1000;"
    "const currentTimeSec = Date.now() / 1000;"
    "const currentEpoch = initEpoch + (currentTimeSec - startTimeSec) / epochLength;"
    "try {"
    "const res = await Functions.makeHttpRequest({"
    "url: `https://api.blockberry.one/walrus-mainnet/v1/blobs/${base64}`,"
    "headers: { 'accept': '*/*', 'x-api-key': apiKey },"
    "timeout: 5000"
    "});"
    "if (res.error || res.status !== 200) return encodeResponse(0, 0);"
    "const { endEpoch } = res.data;"
    "const status = endEpoch > currentEpoch ? 2 : 1;"
    "const endTimestamp = Math.floor(startTimeSec + (endEpoch - initEpoch) * epochLength);"
    "return encodeResponse(status, endTimestamp);"
    "} catch (e) {"
    "return encodeResponse(0, 0);"
    "}";

    constructor(address router) FunctionsClient(router) ConfirmedOwner(msg.sender) {}

    function setProxy(address _proxy) external onlyOwner {
        proxy = _proxy;
    }

    // 允许随时更新 API Key
    function setApiKey(string calldata _apiKey) external onlyOwner {
        apiKey = _apiKey;
    }

    /**
     * @notice 执行请求
     * @param args [blobIdHex] 
     */
    function executeRequest(string[] memory args, uint64 subscriptionId) external returns (bytes32 requestId) {
        require(msg.sender == proxy, "Only proxy");
        
        FunctionsRequest.Request memory req;
        req.initializeRequestForInlineJavaScript(source);
        
        // 构建包含 API Key 的参数列表
        // args[0] = blobIdHex, args[1] = apiKey
        string[] memory finalArgs = new string[](2);
        finalArgs[0] = args[0];
        finalArgs[1] = apiKey;
        
        req.setArgs(finalArgs);
        
        requestId = _sendRequest(req.encodeCBOR(), subscriptionId, gasLimit, donID);
    }

    function fulfillRequest(bytes32 requestId, bytes memory response, bytes memory err) internal override {
        IProxyCallback(proxy).handleResponse(requestId, response, err);
    }
}