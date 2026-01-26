// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {FunctionsClient} from "@chainlink/contracts@1.5.0/src/v0.8/functions/v1_0_0/FunctionsClient.sol";
import {FunctionsRequest} from "@chainlink/contracts@1.5.0/src/v0.8/functions/v1_0_0/libraries/FunctionsRequest.sol";
import {ConfirmedOwner} from "@chainlink/contracts@1.5.0/src/v0.8/shared/access/ConfirmedOwner.sol";

interface IProxyCallback {
    function handleResponse(bytes32 requestId, bytes memory response, bytes memory err) external;
}

contract LighthouseFunctionsConsumer is FunctionsClient, ConfirmedOwner {
    using FunctionsRequest for FunctionsRequest.Request;

    bytes32 public donID = 0x66756e2d617262697472756d2d7365706f6c69612d3100000000000000000000;
    uint32 public gasLimit = 300000;
    address public proxy;

    // JavaScript Source 
    string source = 
        "const cid = args[0];"
        "const ipfsReq = Functions.makeHttpRequest({url:`https://ipfs.io/ipfs/${cid}?format=dag-json`,headers:{'Accept':'application/vnd.ipld.dag-json'},timeout:4000});"
        "const lhReq = Functions.makeHttpRequest({url:`https://api.lighthouse.storage/api/lighthouse/deal_status?cid=${cid}`,timeout:4000});"
        "const encodeResponse = (s, t) => {"
        "  const res = new Uint8Array(64); const v = new DataView(res.buffer);"
        "  v.setUint32(28, s); v.setUint32(60, t); return res;"
        "};"
        "try {"
        "  const [ipfsRes, lhRes] = await Promise.all([ipfsReq, lhReq]);"
        "  if (ipfsRes.error || ipfsRes.status !== 200) return encodeResponse(2, 0);"
        "  let status = 1; let endTime = 0;"
        "  if (!lhRes.error && Array.isArray(lhRes.data) && lhRes.data.length > 0) {"
        "    const dealId = lhRes.data[lhRes.data.length - 1].DealID;"
        "    const ffRes = await Functions.makeHttpRequest({url:`https://filfox.info/api/v1/deal/${dealId}`,timeout:3000});"
        "    if (!ffRes.error && ffRes.status === 200) { status = 0; endTime = ffRes.data.endTimestamp; }"
        "  }"
        "  return encodeResponse(status, endTime);"
        "} catch (e) { return encodeResponse(2, 0); }";

    constructor(address router) FunctionsClient(router) ConfirmedOwner(msg.sender) {}

    function setProxy(address _proxy) external onlyOwner {
        proxy = _proxy;
    }

    function executeRequest(string[] memory args, uint64 subscriptionId) external returns (bytes32 requestId) {
        require(msg.sender == proxy, "Only proxy");
        FunctionsRequest.Request memory req;
        req.initializeRequestForInlineJavaScript(source);
        if (args.length > 0) req.setArgs(args);
        requestId = _sendRequest(req.encodeCBOR(), subscriptionId, gasLimit, donID);
    }

    function fulfillRequest(bytes32 requestId, bytes memory response, bytes memory err) internal override {
        IProxyCallback(proxy).handleResponse(requestId, response, err);
    }
}