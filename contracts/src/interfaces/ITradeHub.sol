// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

interface ITradeHub {
    function reportListEvent(bytes32 saleId, bytes memory dataCommitment, uint256 price, uint256 version) external;
    function reportUpdateEvent(bytes32 saleId, bytes memory dataCommitment, uint256 newPrice, uint256 version) external;
    function reportDelistEvent(bytes32 saleId) external;
    function reportPurchaseEvent(bytes32 saleId, bytes memory dataCommitment, address buyer, uint256 price) external;
    function reportSettleEvent(address buyer, bytes32 saleId, bytes memory dataCommitment) external;
    function reportRefundEvent(address buyer, bytes32 saleId, bytes memory dataCommitment, uint256 amount) external;
}