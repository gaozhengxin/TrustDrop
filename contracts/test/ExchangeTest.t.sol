// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./BaseTest.t.sol";

contract ExchangeTest is BaseTest {
    function _oracleReport(
        bytes32 requestId,
        bytes memory cCipher,
        uint256 status,
        uint256 endTime
    ) internal pure returns (bytes memory) {
        return abi.encode(requestId, cCipher, status, endTime, "");
    }

    function test_FullSuccessFlow() public {
        ExchangeChannelImplementation channel = createChannel();

        // --- 1. 上架 ---
        bytes32 saleId = channel.getNextSaleId();
        vm.prank(owner);
        Types.DataCommitment memory cid = Types.DataCommitment(
            hex"123456789012345678901234567890123456"
        );
        channel.listFile(cid, 1 ether, "Test Data");
        bytes32 version = channel.saleVersions(saleId);

        // --- 2. 购买 (记录购买时的准确数据) ---
        uint256 purchaseTime = block.timestamp; // 记录时间戳
        uint256 deadline = purchaseTime + 10 days;
        Types.Hash vssKey = Types.Hash.wrap(bytes32(uint256(1)));

        vm.deal(buyer, 2 ether);
        vm.prank(buyer);
        channel.purchase{value: 1 ether}(
            saleId,
            version,
            1 ether,
            deadline,
            cid.data,
            vssKey,
            Types.Cipher32.wrap(0)
        );

        // 构造一个与合约内部存储完全一致的 ExchangeInfo
        ExchangeInfo memory info = ExchangeInfo({
            saleDigest: saleId,
            price: 1 ether,
            initTime: purchaseTime, // 必须与购买时一致
            deadline: deadline,
            dataCommitment: cid.data,
            vssKeyCommitment: vssKey
        });

        // --- 3. 卖家 Fulfill ---
        bytes memory cCipher = hex"6666";
        vm.prank(owner);
        channel.fulfill(
            buyer,
            info,
            version,
            VSSArgs(Types.Cipher32.wrap(0), "", ""),
            VDDArgs("", "", cCipher)
        );

        // --- 4. Oracle 回调 ---
        vm.prank(address(oracleProxy));
        channel.onResponse(cCipher, abi.encode(2, purchaseTime + 20 days));

        // --- 5. 结算 ---
        vm.warp(purchaseTime + 8 days); // 越过 7 天窗口

        uint256 balBefore = owner.balance;
        channel.settle(buyer, info, version, cCipher);

        // 验证余额增加 (1 ETH = 10^18 wei)
        assertEq(
            owner.balance,
            balBefore + 1 ether,
            "Owner balance should increase by 1 ETH"
        );
    }

    function test_OracleProxyReportUpdatesChannelAndAllowsSettle() public {
        vm.warp(1_000);
        ExchangeChannelImplementation channel = createChannel();

        bytes32 saleId = channel.getNextSaleId();
        vm.prank(owner);
        Types.DataCommitment memory cid = Types.DataCommitment(
            hex"123456789012345678901234567890123456"
        );
        channel.listFile(cid, 1 ether, "Test Data");
        bytes32 version = channel.saleVersions(saleId);

        uint256 purchaseTime = block.timestamp;
        uint256 deadline = purchaseTime + 10 days;
        Types.Hash vssKey = Types.Hash.wrap(bytes32(uint256(1)));

        vm.prank(buyer);
        channel.purchase{value: 1 ether}(
            saleId,
            version,
            1 ether,
            deadline,
            cid.data,
            vssKey,
            Types.Cipher32.wrap(0)
        );

        ExchangeInfo memory info = ExchangeInfo({
            saleDigest: saleId,
            price: 1 ether,
            initTime: purchaseTime,
            deadline: deadline,
            dataCommitment: cid.data,
            vssKeyCommitment: vssKey
        });

        bytes memory cCipher = hex"6666";
        uint256 nonceBefore = oracleProxy.nonce();

        vm.prank(owner);
        channel.fulfill(
            buyer,
            info,
            version,
            VSSArgs(Types.Cipher32.wrap(0), "", ""),
            VDDArgs("", "", cCipher)
        );

        require(channel.vddVerified(cCipher), "VDD should be marked verified");

        bytes32 requestId;
        uint256 nonceAfter = oracleProxy.nonce();
        for (
            uint256 requestNonce = nonceBefore;
            requestNonce < nonceAfter;
            requestNonce++
        ) {
            bytes32 candidate = keccak256(
                abi.encode(
                    block.chainid,
                    address(oracleProxy),
                    address(channel),
                    cCipher,
                    requestNonce
                )
            );
            (, address client, , bool fulfilled) = oracleProxy.requests(
                candidate
            );
            if (client == address(channel) && !fulfilled) {
                requestId = candidate;
                break;
            }
        }
        require(requestId != bytes32(0), "OracleRequested log missing");

        vm.prank(owner);
        oracleProxy.submitCentralizedReport(
            _oracleReport(requestId, cCipher, 2, purchaseTime + 20 days)
        );

        assertEq(
            channel.oracleSuccessUntil(cCipher),
            purchaseTime + 20 days,
            "OracleProxy report should update oracleSuccessUntil"
        );

        vm.warp(purchaseTime + 8 days);
        uint256 balBefore = owner.balance;
        channel.settle(buyer, info, version, cCipher);

        assertEq(owner.balance, balBefore + 1 ether);
    }

    // 内部辅助，构造 ExchangeInfo
    function _getMockInfo(
        bytes32 sId,
        bytes memory data
    ) internal view returns (ExchangeInfo memory) {
        return
            ExchangeInfo(
                sId,
                1 ether,
                block.timestamp - 8 days,
                block.timestamp + 2 days,
                data,
                Types.Hash.wrap(bytes32(uint256(1)))
            );
    }
}
