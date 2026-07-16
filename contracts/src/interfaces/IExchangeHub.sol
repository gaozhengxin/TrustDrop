// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {ExchangeInfo} from "../ExchangeChannel.sol";

interface IExchangeHub {
    function reportListEvent(bytes32 saleId, bytes memory dataCommitment, uint256 price, bytes32 version, string memory info) external;
    function reportUpdateEvent(bytes32 saleId, bytes memory dataCommitment, uint256 newPrice, bytes32 version, string memory info) external;
    function reportDelistEvent(bytes32 saleId) external;
    function reportPurchaseEvent(bytes32 saleId, bytes memory dataCommitment, address buyer, uint256 price, ExchangeInfo memory exchangeInfo) external;
    function reportSettleEvent(address buyer, bytes32 saleId, bytes memory dataCommitment) external;
    function reportRefundEvent(address buyer, bytes32 saleId, bytes memory dataCommitment, uint256 amount) external;
}
