// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./BaseTest.t.sol";

contract ProxyIsolationTest is BaseTest {
    ExchangeChannelImplementation public channelA;
    ExchangeChannelImplementation public channelB;

    // 定义第二个卖家的地址，用于权限隔离验证
    address public ownerB = address(0x99);

    /**
     * @notice 初始化测试环境
     * 必须 override 父类的 setUp 并调用 super.setUp() 以确保基础合约已部署
     */
    function setUp() public override {
        super.setUp();

        vm.deal(ownerB, 10 ether);

        // 1. 创建第一个数据仓 (Owner 是默认的 address(0x1))
        channelA = createChannel();

        // 2. 创建第二个数据仓 (Owner 设为 0x99)
        // 验证 Hub 是否能根据不同的 msg.sender 分配正确的权限
        vm.prank(ownerB);
        address proxyB = hub.createExchangeChannel(Types.Pubkey(hex"02"));
        channelB = ExchangeChannelImplementation(proxyB);
    }

    /**
     * @notice 测试 1：基础状态变量隔离
     * 验证 Implementation 的逻辑指针是否能准确击中不同 Proxy 的独立存储插槽
     */
    function test_VariableIsolation() public {
        // --- 操作 Channel A ---
        vm.prank(owner);
        bytes32 commitmentA = bytes32(uint256(0xAAAA));
        channelA.submitDataKeyCommitment(Types.Hash.wrap(commitmentA));

        // --- 操作 Channel B ---
        vm.prank(ownerB);
        bytes32 commitmentB = bytes32(uint256(0xBBBB));
        channelB.submitDataKeyCommitment(Types.Hash.wrap(commitmentB));

        // --- 断言隔离性 ---
        assertEq(
            Types.Hash.unwrap(channelA.dataKeyCommitment()),
            commitmentA,
            "Channel A data corrupted by B"
        );
        assertEq(
            Types.Hash.unwrap(channelB.dataKeyCommitment()),
            commitmentB,
            "Channel B data corrupted by A"
        );

        // 验证 A 的主人无法修改 B 的数据
        vm.prank(owner);
        vm.expectRevert("Not owner");
        channelB.submitDataKeyCommitment(
            Types.Hash.wrap(bytes32(uint256(0xCCCC)))
        );
    }

    /**
     * @notice 测试 2：映射表 (Mapping) 独立性隔离
     * 验证复杂的映射结构（如 isRegistered, audienceIndex, privyBitmaps）在不同实例中互不干扰
     */
    function test_MappingIsolation() public {
        // --- 1. 买家在 Channel A 中注册 ---
        vm.prank(buyer);
        Types.Hash vssKeyA = Types.Hash.wrap(bytes32(uint256(0x777)));
        channelA.join(vssKeyA, Types.Cipher32.wrap(0));

        // 验证 A 的注册状态
        assertTrue(
            channelA.isRegistered(buyer),
            "Buyer should be registered in A"
        );
        assertEq(channelA.audienceIndex(buyer), 0, "Index in A should be 0");

        // 核心验证：验证 B 的映射表完全为空（即便逻辑代码共享，数据也必须物理隔离）
        assertFalse(
            channelB.isRegistered(buyer),
            "Mapping leakage: Buyer found in B"
        );
        assertEq(
            channelB.audienceIndex(buyer),
            0,
            "Index in B should be default zero"
        );

        // --- 2. 验证 PrivyBitmaps 的位图隔离 ---
        // 模拟卖家 A 为买家发放密钥
        vm.prank(owner);
        address[] memory aud = new address[](1);
        aud[0] = buyer;
        Types.Cipher32[] memory keys = new Types.Cipher32[](1);
        keys[0] = Types.Cipher32.wrap(0);

        // 这里的 shareDataKey 会更新 channelA 的 mapping(uint256 => uint256) privyBitmaps
        channelA.shareDataKey("", "", aud, keys);

        // 断言 A 中已授权，B 中依然未授权
        assertTrue(channelA.isPrivy(buyer), "Buyer must be privy in A");
        assertFalse(
            channelB.isPrivy(buyer),
            "Bit leaked: Buyer must NOT be privy in B"
        );
    }

    /**
     * @notice 测试 3：资金账户隔离
     * 验证不同代理合约地址下的 ETH 余额完全独立
     */
    function test_BalanceIsolation() public {
        // --- 1. 分别注资 ---
        vm.deal(address(channelA), 10 ether);
        vm.deal(address(channelB), 50 ether);

        assertEq(address(channelA).balance, 10 ether, "A balance init fail");
        assertEq(address(channelB).balance, 50 ether, "B balance init fail");

        // --- 2. 模拟 Channel A 的结算操作 ---
        // 此时我们直接调用内部逻辑，模拟 A 支出一笔钱给卖家
        // 逻辑母机执行代码，但 msg.value 和 balance 语境都在 channelA
        vm.prank(address(channelA));
        (bool success, ) = payable(owner).call{value: 5 ether}("");
        require(success, "Transfer from A failed");

        // --- 3. 验证隔离结果 ---
        assertEq(
            address(channelA).balance,
            5 ether,
            "A balance should decrease"
        );
        assertEq(
            address(channelB).balance,
            50 ether,
            "B balance must remain 50 ether"
        );
    }

    /**
     * @notice 测试 4：Nonce 与插槽计数隔离
     * 验证业务流程导致的计数器自增是彼此独立的
     */
    function test_NonceIsolation() public {
        // 使用 startPrank 开启持久化身份模拟
        vm.startPrank(owner);

        Types.DataCommitment memory cid = Types.DataCommitment(
            hex"123456789012345678901234567890123456"
        );

        // 第一次调用：msg.sender 是 owner，成功
        channelA.listFile(cid, 1 ether, "info1");

        // 第二次调用：因为使用了 startPrank，msg.sender 依然是 owner，成功
        channelA.listFile(cid, 2 ether, "info2");

        // 结束身份模拟
        vm.stopPrank();

        // 验证断言：A 应该自增到 2，B 应该保持 0
        assertEq(channelA.nonce(), 2, "A nonce failed to increment");
        assertEq(
            channelB.nonce(),
            0,
            "B nonce should stay zero (Isolation Check)"
        );
    }
}
