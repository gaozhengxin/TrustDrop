// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Ownable} from "../lib/Ownable.sol";

contract OracleProxy is Ownable {
    enum OracleMode {
        Centralized,
        ChainlinkCRE
    }

    struct RequestContext {
        bytes cid;
        address client;
        OracleMode mode;
        bool fulfilled;
    }

    address public controller;
    address public centralizedOracleSigner;
    address public creForwarder;
    OracleMode public defaultMode;
    uint256 public nonce;

    mapping(bytes32 => RequestContext) public requests;
    mapping(address => bool) public whiteList;

    event OracleRequested(
        bytes32 indexed requestId,
        address indexed client,
        bytes cid,
        uint256 nonce,
        OracleMode mode
    );
    event CallbackResult(bytes32 indexed requestId, bool success);
    event ControllerUpdated(address indexed controller);
    event CentralizedOracleSignerUpdated(address indexed signer);
    event CREForwarderUpdated(address indexed forwarder);
    event DefaultModeUpdated(OracleMode mode);
    event WhitelistUpdated(address indexed caller, bool allowed);

    modifier onlyController() {
        require(msg.sender == controller, "Not controller");
        _;
    }

    modifier onlyWhitelisted() {
        require(whiteList[msg.sender], "Not whitelisted");
        _;
    }

    constructor(
        address _centralizedOracleSigner,
        address _creForwarder
    ) Ownable(msg.sender) {
        centralizedOracleSigner = _centralizedOracleSigner;
        creForwarder = _creForwarder;
        defaultMode = OracleMode.Centralized;
    }

    function setController(address newController) public onlyOwner {
        controller = newController;
        emit ControllerUpdated(newController);
    }

    function setCentralizedOracleSigner(address signer) external onlyOwner {
        centralizedOracleSigner = signer;
        emit CentralizedOracleSignerUpdated(signer);
    }

    function setCREForwarder(address forwarder) external onlyOwner {
        creForwarder = forwarder;
        emit CREForwarderUpdated(forwarder);
    }

    function setDefaultMode(OracleMode mode) external onlyOwner {
        defaultMode = mode;
        emit DefaultModeUpdated(mode);
    }

    function request(
        bytes memory cCipher,
        address callback
    ) external onlyWhitelisted {
        require(callback != address(0), "Invalid callback");

        uint256 requestNonce = nonce++;
        bytes32 requestId = keccak256(
            abi.encode(
                block.chainid,
                address(this),
                callback,
                cCipher,
                requestNonce
            )
        );

        requests[requestId] = RequestContext({
            cid: cCipher,
            client: callback,
            mode: defaultMode,
            fulfilled: false
        });

        emit OracleRequested(
            requestId,
            callback,
            cCipher,
            requestNonce,
            defaultMode
        );
    }

    function submitCentralizedReport(bytes calldata report) external {
        require(centralizedOracleSigner != address(0), "Signer not set");
        require(msg.sender == centralizedOracleSigner, "Unauthorized signer");
        _handleOracleReport(report, OracleMode.Centralized);
    }

    function onReport(
        bytes calldata,
        bytes calldata report
    ) external {
        require(creForwarder != address(0), "CRE forwarder not set");
        require(msg.sender == creForwarder, "Unauthorized forwarder");
        _handleOracleReport(report, OracleMode.ChainlinkCRE);
    }

    function setWhitelist(
        address caller,
        bool allowed
    ) external onlyController {
        whiteList[caller] = allowed;
        emit WhitelistUpdated(caller, allowed);
    }

    function _handleOracleReport(
        bytes calldata report,
        OracleMode expectedMode
    ) internal {
        (
            bytes32 requestId,
            bytes memory cCipher,
            uint256 status,
            uint256 endTime,
            bytes memory err
        ) = abi.decode(report, (bytes32, bytes, uint256, uint256, bytes));

        RequestContext storage ctx = requests[requestId];
        require(ctx.client != address(0), "Unknown request");
        require(!ctx.fulfilled, "Request fulfilled");
        require(ctx.mode == expectedMode, "Wrong oracle mode");
        require(keccak256(ctx.cid) == keccak256(cCipher), "CID mismatch");
        require(status <= 2, "Invalid status");

        bool success;
        if (err.length > 0) {
            (success, ) = ctx.client.call(
                abi.encodeWithSignature(
                    "onOracleError(bytes,bytes)",
                    ctx.cid,
                    err
                )
            );
        } else {
            bytes memory response = abi.encode(status, endTime);
            (success, ) = ctx.client.call(
                abi.encodeWithSignature(
                    "onResponse(bytes,bytes)",
                    ctx.cid,
                    response
                )
            );
        }

        require(success, "Callback failed");
        ctx.fulfilled = true;

        emit CallbackResult(requestId, success);
    }
}
