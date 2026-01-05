// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./TradeChannel.sol";
import "./interfaces/ITradeHub.sol";

contract TradeHub is ITradeHub {
    mapping(address => bool) public isRegisteredChannel;

    event TradeChannelCreated(address indexed owner, address indexed channel);
    event SaleListed(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 price,
        uint256 version
    );
    event SaleUpdated(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 newPrice,
        uint256 version
    );
    event SaleDelisted(address indexed channel, bytes32 indexed saleId);
    event PurchaseEvent(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        address indexed buyer,
        uint256 price
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

    modifier onlyRegisteredChannel() {
        require(isRegisteredChannel[msg.sender], "Unauthorized channel");
        _;
    }

    function createTradeChannel(
        Types.Pubkey memory ownerPubKey,
        address oracleWrapper
    ) public returns (address) {
        // TradeHub -> TradeChannel (One-way dependency in bytecode)
        TradeChannel newChannel = new TradeChannel(
            ownerPubKey,
            oracleWrapper,
            address(this)
        );
        address channelAddr = address(newChannel);
        isRegisteredChannel[channelAddr] = true;

        emit TradeChannelCreated(msg.sender, channelAddr);
        return channelAddr;
    }

    function reportListEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 price,
        uint256 version
    ) external override onlyRegisteredChannel {
        emit SaleListed(msg.sender, saleId, dataCommitment, price, version);
    }

    function reportUpdateEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 newPrice,
        uint256 version
    ) external override onlyRegisteredChannel {
        emit SaleUpdated(msg.sender, saleId, dataCommitment, newPrice, version);
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
        uint256 price
    ) external override onlyRegisteredChannel {
        emit PurchaseEvent(msg.sender, saleId, dataCommitment, buyer, price);
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
