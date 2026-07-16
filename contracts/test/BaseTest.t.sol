// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "forge-std/Test.sol";
import "../src/ExchangeChannel.sol";
import "../src/ExchangeHub.sol";
import "../src/oracle/OracleProxy.sol";

contract BaseTest is Test {
    ExchangeChannelImplementation public implementation;
    ExchangeHub public hub;
    OracleProxy public oracleProxy;
    MockVerifier public verifier;

    address public owner = address(0x1);
    address public buyer = address(0x2);
    address public hubOwner = address(0x3);

    function setUp() public virtual {
        vm.deal(owner, 100 ether);
        vm.deal(buyer, 100 ether);

        vm.startPrank(owner);

        // 1. 部署辅助合约
        verifier = new MockVerifier();
        oracleProxy = new OracleProxy(owner, address(0xbeef));

        // 2. 部署逻辑母机 (根据你的实现类构造函数)
        // 注意：母机部署时的参数在代理模式下不重要，但需要符合构造函数定义
        implementation = new ExchangeChannelImplementation(
            Types.Pubkey(hex"00"),
            address(0),
            address(0),
            owner,
            address(0),
            address(0)
        );

        // 3. 部署 Hub (匹配 4 参数 ABI)
        hub = new ExchangeHub(
            address(oracleProxy),
            address(verifier),
            address(verifier), // vddVerifier
            address(implementation)
        );
        oracleProxy.setController(address(hub));

        vm.stopPrank();
    }

    // 辅助工具：根据最新 Hub ABI 创建 Channel
    function createChannel() public returns (ExchangeChannelImplementation) {
        vm.prank(owner);
        address proxy = hub.createExchangeChannel(Types.Pubkey(hex"01"));
        return ExchangeChannelImplementation(proxy);
    }

    function _setupPurchase(
        ExchangeChannelImplementation channel,
        bytes32 saleId,
        bytes memory data,
        uint256 price,
        uint256 deadline,
        Types.Hash vssKey
    ) internal returns (ExchangeInfo memory) {
        uint256 initTime = block.timestamp;
        bytes32 version = channel.saleVersions(saleId);

        vm.prank(buyer);
        channel.purchase{value: price}(
            saleId,
            version,
            price,
            deadline,
            data,
            vssKey,
            hex"010203"
        );

        // 返回精确的结构体，避免手动构造时参数错位
        return
            ExchangeInfo({
                saleDigest: saleId,
                price: price,
                initTime: initTime,
                deadline: deadline,
                dataCommitment: data,
                vssKeyCommitment: vssKey
            });
    }
}
