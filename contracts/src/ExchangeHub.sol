// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./ExchangeChannel.sol";
import "./interfaces/IExchangeHub.sol";
import "./interfaces/IOracleProxy.sol";
import "./interfaces/IVerifier.sol";
import "./lib/Ownable.sol";
import {ExchangeInfo} from "./ExchangeChannel.sol";

import "@openzeppelin/contracts/proxy/Clones.sol";

contract ExchangeHub is IExchangeHub, Ownable {
    address public immutable implementation;

    IOracleProxy public oracleWrapper;
    IVSSVerifier public vssVerifier;
    IVDDVerifier public vddVerifier;

    mapping(address => bool) public isRegisteredChannel;

    event ExchangeChannelCreated(
        address indexed owner,
        address indexed channel
    );
    event SaleListed(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 price,
        bytes32 version,
        string info
    );
    event SaleUpdated(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 newPrice,
        bytes32 version,
        string info
    );
    event SaleDelisted(address indexed channel, bytes32 indexed saleId);
    event PurchaseEvent(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        address indexed buyer,
        uint256 price,
        ExchangeInfo exchangeInfo
    );
    event SettleEvent(
        address indexed channel,
        address indexed buyer,
        bytes32 indexed saleId,
        bytes dataCommitment
    );
    event RefundEvent(
        address indexed channel,
        address indexed buyer,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 amount
    );
    event OracleWrapperUpdated(address indexed oracleWrapper);
    event VSSVerifierUpdated(address indexed verifier);
    event VDDVerifierUpdated(address indexed verifier);

    modifier onlyRegisteredChannel() {
        require(isRegisteredChannel[msg.sender], "Unauthorized channel");
        _;
    }

    constructor(
        address _oracleWrapper,
        address _vssVerifier,
        address _vddVerifier,
        address _implementation
    ) Ownable(msg.sender) {
        oracleWrapper = IOracleProxy(_oracleWrapper);
        vssVerifier = IVSSVerifier(_vssVerifier);
        vddVerifier = IVDDVerifier(_vddVerifier);
        implementation = _implementation;
    }

    function createExchangeChannel(
        Types.Pubkey memory ownerPubKey
    ) public returns (address) {
        address proxy = Clones.clone(implementation);
        ExchangeChannelStorage(proxy).initialize(
            ownerPubKey,
            address(oracleWrapper),
            address(this),
            msg.sender, // owner
            address(vssVerifier),
            address(vddVerifier)
        );

        isRegisteredChannel[proxy] = true;
        oracleWrapper.setWhitelist(proxy, true);
        emit ExchangeChannelCreated(msg.sender, proxy);
        return proxy;
    }

    function setOracleWrapper(address _oracleWrapper) external onlyOwner {
        require(_oracleWrapper != address(0), "Invalid oracle");
        oracleWrapper = IOracleProxy(_oracleWrapper);
        emit OracleWrapperUpdated(_oracleWrapper);
    }

    function setVSSVerifier(address _vssVerifier) external onlyOwner {
        require(_vssVerifier != address(0), "Invalid VSS verifier");
        vssVerifier = IVSSVerifier(_vssVerifier);
        emit VSSVerifierUpdated(_vssVerifier);
    }

    function setVDDVerifier(address _vddVerifier) external onlyOwner {
        require(_vddVerifier != address(0), "Invalid VDD verifier");
        vddVerifier = IVDDVerifier(_vddVerifier);
        emit VDDVerifierUpdated(_vddVerifier);
    }

    function reportListEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 price,
        bytes32 version,
        string memory info
    ) external override onlyRegisteredChannel {
        emit SaleListed(
            msg.sender,
            saleId,
            dataCommitment,
            price,
            version,
            info
        );
    }

    function reportUpdateEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 newPrice,
        bytes32 version,
        string memory info
    ) external override onlyRegisteredChannel {
        emit SaleUpdated(
            msg.sender,
            saleId,
            dataCommitment,
            newPrice,
            version,
            info
        );
    }

    function reportDelistEvent(
        bytes32 saleId
    ) external override onlyRegisteredChannel {
        emit SaleDelisted(msg.sender, saleId);
    }

    function reportPurchaseEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        address buyer,
        uint256 price,
        ExchangeInfo memory exchangeInfo
    ) external override onlyRegisteredChannel {
        emit PurchaseEvent(
            msg.sender,
            saleId,
            dataCommitment,
            buyer,
            price,
            exchangeInfo
        );
    }

    function reportSettleEvent(
        address buyer,
        bytes32 saleId,
        bytes memory dataCommitment
    ) external override onlyRegisteredChannel {
        emit SettleEvent(msg.sender, buyer, saleId, dataCommitment);
    }

    function reportRefundEvent(
        address buyer,
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 amount
    ) external override onlyRegisteredChannel {
        emit RefundEvent(msg.sender, buyer, saleId, dataCommitment, amount);
    }
}
