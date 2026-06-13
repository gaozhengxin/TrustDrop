pragma solidity ^0.8.13;

import "./BaseTest.t.sol";

contract SecurityTest is BaseTest {
    ExchangeChannelImplementation public channel;

    function setUp() public override {
        super.setUp();
        channel = createChannel();
    }

    // --- 1. 初始化锁测试 ---
    function test_ReinitializationAttack() public {
        // 尝试对已经初始化的代理再次进行初始化
        vm.expectRevert("Already initialized");
        channel.initialize(
            Types.Pubkey(hex"00"),
            address(0),
            address(0),
            address(0),
            address(0),
            address(0)
        );
    }

    // --- 2. 核心功能权限测试 (onlyOwner) ---
    function test_AccessControl_OnlyOwner() public {
        vm.prank(buyer); // 模拟非 Owner 身份

        // 尝试上架文件
        Types.DataCommitment memory cid = Types.DataCommitment(hex"1234");
        vm.expectRevert("Not owner");
        channel.listFile(cid, 1 ether, "Hacked");

        // 尝试提交 VDD 证明
        vm.expectRevert("Not owner");
        channel.submitVDDProof("", "", "", "");
    }

    // --- 3. Oracle 回调门禁测试 ---
    function test_AccessControl_OnlyOracle() public {
        vm.prank(owner); // 即使是 Owner 也不能跳过 OracleProxy 伪造回调
        vm.expectRevert("Only oracle proxy");
        channel.onResponse(hex"00", abi.encode(2, 999999));

        vm.prank(address(0xdead)); // 黑客尝试伪造
        vm.expectRevert("Only oracle proxy");
        channel.onResponse(hex"00", abi.encode(2, 999999));
    }

    // --- 4. 结算前提测试 (Condition Lock) ---
    function test_SettlementCondition_MustBePrivy() public {
        // 1. 准备环境：创建 Channel 并上架
        ExchangeChannelImplementation channel = createChannel();
        bytes32 saleId = channel.getNextSaleId();
        vm.prank(owner);
        Types.DataCommitment memory cid = Types.DataCommitment(
            hex"123456789012345678901234567890123456"
        );
        channel.listFile(cid, 1 ether, "Sensitive Data");
        bytes32 version = channel.saleVersions(saleId);
        uint256 balanceBefore = owner.balance;

        // 2. 买家购买
        vm.deal(buyer, 1 ether);
        vm.prank(buyer);
        Types.Hash vssKeyCommitment = Types.Hash.wrap(bytes32(uint256(0x111)));
        channel.purchase{value: 1 ether}(
            saleId,
            version,
            1 ether,
            block.timestamp + 10 days,
            cid.data,
            vssKeyCommitment,
            Types.Cipher32.wrap(0)
        );

        // 3. 模拟 Oracle 响应：假设存储已经验证成功 (Status 2)
        bytes memory cCipher = hex"aabbcc";
        vm.prank(owner);
        channel.submitVDDProof("", "", cid.data, cCipher); // 先触发 ZK 验证标记 vddVerified

        vm.prank(address(oracleProxy));
        channel.onResponse(cCipher, abi.encode(2, block.timestamp + 20 days));

        // 构造 ExchangeInfo 结构体用于后续调用
        ExchangeInfo memory info = ExchangeInfo({
            saleDigest: saleId,
            price: 1 ether,
            initTime: block.timestamp,
            deadline: block.timestamp + 10 days,
            dataCommitment: cid.data,
            vssKeyCommitment: vssKeyCommitment
        });

        // 4. 时间旅行：跳过 7 天生存窗口
        vm.warp(block.timestamp + 8 days);

        // 5. 核心测试点：
        // 虽然 Oracle 过了，时间也到了，但卖家尚未 fulfill (即未 shareDataKey)，买家还不是 Privy
        assertFalse(channel.isPrivy(buyer), "Buyer should not be privy yet");

        vm.prank(buyer);
        vm.expectRevert("Buyer not privy"); // 这里必须拦截
        channel.settle(buyer, info, version, cCipher);

        // 6. 验证后续：卖家一旦 fulfill，结算即可通过
        vm.prank(owner);
        VSSArgs memory vss = VSSArgs(Types.Cipher32.wrap(0), "", ""); // Mock 证明
        VDDArgs memory vdd = VDDArgs("", "", cCipher);
        channel.fulfill(buyer, info, version, vss, vdd);

        assertTrue(channel.isPrivy(buyer), "Buyer should be privy now");

        vm.prank(buyer);
        channel.settle(buyer, info, version, cCipher);
        assertEq(
            owner.balance,
            balanceBefore + 1 ether,
            "Owner should receive payment"
        );
    }
}
