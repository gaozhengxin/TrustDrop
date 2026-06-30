pragma solidity ^0.8.13;

import "./BaseTest.t.sol";

/**
 * @title BitmapTest
 * @notice 专门验证 privyBitmaps 在跨越桶（Bucket）边界时的索引逻辑
 */
contract BitmapTest is BaseTest {
    ExchangeChannelImplementation public channel;

    function setUp() public override {
        super.setUp();
        channel = createChannel();
    }

    /**
     * @notice 验证位图从 Bucket 0 到 Bucket 1 的平滑过渡
     * 测试路径：填充第 255 位（桶0末尾）和第 256 位（桶1起始）
     */
    function test_BitmapBucketTransition() public {
        // --- 1. 准备数据：注册足够多的用户以跨越边界 ---
        // 我们需要注册 257 个用户（0 到 256）
        // 为了节省测试 Gas，我们重点注册关键位置的用户

        address user255 = address(0x255); // 索引 255
        address user256 = address(0x256); // 索引 256

        // 模拟前 255 个用户加入，占满 Bucket 0 的坑位
        // 实际上只要最后两个加入即可，索引是自增的
        for (uint256 i = 0; i < 255; i++) {
            address dummy = address(uint160(1000 + i));
            vm.prank(dummy);
            channel.join(Types.Hash.wrap(bytes32(i)), Types.Cipher32.wrap(0));
        }

        // 加入第 256 个用户 (Index 255)
        vm.prank(user255);
        channel.join(
            Types.Hash.wrap(bytes32(uint256(255))),
            Types.Cipher32.wrap(0)
        );
        assertEq(
            channel.audienceIndex(user255),
            255,
            "User 255 should have index 255"
        );

        // 加入第 257 个用户 (Index 256) -> 应该进入下一个 Bucket
        vm.prank(user256);
        channel.join(
            Types.Hash.wrap(bytes32(uint256(256))),
            Types.Cipher32.wrap(0)
        );
        assertEq(
            channel.audienceIndex(user256),
            256,
            "User 256 should have index 256"
        );

        // --- 2. 授权操作：Owner 更新位图 ---
        vm.startPrank(owner);

        address[] memory audiences = new address[](2);
        audiences[0] = user255;
        audiences[1] = user256;

        Types.Cipher32[] memory encryptedKeys = new Types.Cipher32[](2);
        encryptedKeys[0] = Types.Cipher32.wrap(bytes32(uint256(0x1)));
        encryptedKeys[1] = Types.Cipher32.wrap(bytes32(uint256(0x2)));

        // 这里的 shareDataKey 会触发位图更新
        // 内部逻辑：privyBitmaps[idx/256] |= (1 << (idx%256))
        channel.shareDataKey("", "", audiences, encryptedKeys);

        vm.stopPrank();

        // --- 3. 验证结果 ---

        // 验证业务接口 isPrivy
        assertTrue(channel.isPrivy(user255), "User 255 should be privy");
        assertTrue(channel.isPrivy(user256), "User 256 should be privy");

        // 验证底层存储 privyBitmaps
        // Bucket 0 的第 255 位应该是 1
        uint256 bucket0 = channel.privyBitmaps(0);
        assertTrue((bucket0 >> 255) & 1 == 1, "Bucket 0 last bit not set");

        // Bucket 1 的第 0 位应该是 1
        uint256 bucket1 = channel.privyBitmaps(1);
        assertEq(bucket1 & 1, 1, "Bucket 1 first bit not set");

        // 确保没有发生位移污染：Bucket 1 目前应该只有这一位是 1
        assertEq(bucket1, 1, "Bucket 1 contains unexpected bits");
    }

    /**
     * @notice 验证未授权用户在跨桶时的隔离性
     */
    function test_BitmapIsolationInNewBucket() public {
        // 直接注册到第 257 个用户
        for (uint256 i = 0; i <= 256; i++) {
            address user = address(uint160(2000 + i));
            vm.prank(user);
            channel.join(Types.Hash.wrap(bytes32(i)), Types.Cipher32.wrap(0));
        }

        address user256 = address(uint160(2000 + 256));

        // 验证即使索引到了新桶，如果没有被 shareDataKey，依然不是 Privy
        assertFalse(
            channel.isPrivy(user256),
            "User 256 should not be privy initially"
        );
        assertEq(channel.privyBitmaps(1), 0, "Bucket 1 should be empty");
    }

    function test_VssReuseHelpersForBatching() public {
        address userA = address(0xA11CE);
        address userB = address(0xB0B);

        vm.prank(userA);
        channel.join(
            Types.Hash.wrap(bytes32(uint256(0xaaa))),
            Types.Cipher32.wrap(bytes32(uint256(0x111)))
        );
        vm.prank(userB);
        channel.join(
            Types.Hash.wrap(bytes32(uint256(0xbbb))),
            Types.Cipher32.wrap(bytes32(uint256(0x222)))
        );

        assertEq(channel.audienceCount(), 2, "audience count mismatch");
        assertTrue(channel.needsVSS(userA), "new audience should need VSS");
        assertTrue(channel.needsVSS(userB), "new audience should need VSS");

        address[] memory audiences = new address[](2);
        audiences[0] = userA;
        audiences[1] = userB;

        bytes32[] memory commitments = channel.getAudienceVssKeyCommitments(
            audiences
        );
        assertEq(commitments[0], bytes32(uint256(0xaaa)));
        assertEq(commitments[1], bytes32(uint256(0xbbb)));

        Types.Cipher32[] memory encryptedKeys = new Types.Cipher32[](2);
        encryptedKeys[0] = Types.Cipher32.wrap(bytes32(uint256(0x333)));
        encryptedKeys[1] = Types.Cipher32.wrap(bytes32(uint256(0x444)));

        vm.prank(owner);
        channel.shareDataKey("", "", audiences, encryptedKeys);

        assertFalse(channel.needsVSS(userA), "privy audience should skip VSS");
        assertFalse(channel.needsVSS(userB), "privy audience should skip VSS");
    }
}
