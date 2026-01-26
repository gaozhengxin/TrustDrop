// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./BaseTest.t.sol";

contract NegativePathTest is BaseTest {
    ExchangeChannelImplementation public channel;
    ExchangeInfo public info;
    bytes public cidData = hex"123456789012345678901234567890123456";

    function setUp() public override {
        super.setUp();
        channel = createChannel();

        // 锁定当前 SaleId
        bytes32 targetSaleId = channel.getNextSaleId();

        // 上架
        vm.prank(owner);
        channel.listFile(
            Types.DataCommitment(cidData),
            1 ether,
            "Critical Data"
        );

        // 购买
        info = _setupPurchase(
            channel,
            targetSaleId,
            cidData,
            1 ether,
            block.timestamp + 10 days,
            Types.Hash.wrap(bytes32(uint256(0x999)))
        );
    }

    function test_Negative_OracleStatusZero() public {
        bytes32 currentVersion = channel.saleVersions(info.saleDigest);
        bytes memory cCipher = hex"dead";

        // 1. 提交 VDD 证明
        vm.prank(owner);
        channel.submitVDDProof("", "", cidData, cCipher);

        // 2. 关键修复：让买家成为 Privy，否则 settle 会死在第一行 "Buyer not privy"
        vm.prank(owner);
        address[] memory aud = new address[](1);
        aud[0] = buyer;
        channel.shareDataKey("", "", aud, new Types.Cipher32[](1));

        // 3. 模拟 Oracle 报告不可取回
        vm.prank(address(oracleProxy));
        channel.onResponse(cCipher, abi.encode(0, 0));

        vm.warp(block.timestamp + 8 days);

        // 4. 执行结算，预期报错 Oracle 失败
        vm.prank(buyer);
        vm.expectRevert("Oracle proof expired or missing");
        channel.settle(buyer, info, currentVersion, cCipher);
    }

    function test_Negative_ZKFailure() public {
        bytes32 currentVersion = channel.saleVersions(info.saleDigest);

        vm.mockCall(
            address(verifier),
            abi.encodeWithSelector(0x11047aa4),
            abi.encode(false)
        );

        VSSArgs memory vss = VSSArgs(Types.Cipher32.wrap(0), "", "");
        VDDArgs memory vdd = VDDArgs("", "", hex"6666");

        vm.prank(owner);
        vm.expectRevert("VSS verification failed");
        channel.fulfill(buyer, info, currentVersion, vss, vdd);
    }

    function test_PostDeadline_MutualExclusion() public {
        bytes32 currentVersion = channel.saleVersions(info.saleDigest);

        vm.warp(info.deadline + 1 days);

        // 买家抢先退款
        vm.prank(buyer);
        channel.refund(buyer, info, currentVersion);

        // 卖家随后尝试结算
        vm.prank(owner);
        vm.expectRevert("No Exchange");
        channel.settle(buyer, info, currentVersion, hex"6666");
    }

    function test_PostDeadline_RefundBlocksSettle() public {
        bytes32 currentVersion = channel.saleVersions(info.saleDigest);
        bytes memory cCipher = hex"8888";

        // 1. 满足 Oracle
        vm.prank(owner);
        channel.submitVDDProof("", "", cidData, cCipher);
        vm.prank(address(oracleProxy));
        channel.onResponse(cCipher, abi.encode(2, block.timestamp + 20 days));

        // 2. 满足 Privy
        vm.prank(owner);
        address[] memory aud = new address[](1);
        aud[0] = buyer;
        channel.shareDataKey("", "", aud, new Types.Cipher32[](1));

        vm.warp(info.deadline + 1 hours);

        // 3. 买家抢退
        vm.prank(buyer);
        channel.refund(buyer, info, currentVersion);

        // 4. 卖家结算失败
        vm.prank(owner);
        vm.expectRevert("No Exchange");
        channel.settle(buyer, info, currentVersion, cCipher);
    }
}
