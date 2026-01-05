// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./lib/Types.sol";
import "./interfaces/IVerifier.sol";

contract VSS {
    using Types for *;

    struct AudienceInfo {
        Types.Hash vssKeyCommitment;
        Types.Cipher32 encryptedVssKey;
    }

    // --- zk 验证器 ---
    IVerifier public verifier;

    // --- 状态变量 ---
    address public owner;
    Types.Pubkey public ownerPublicKey;
    Types.Hash public dataKeyCommitment;

    // 全局版本号：每次 shareDataKey 时自增
    uint256 public version;

    // 位图：每个 bit 代表一个观众是否在当前 version 中被授权
    // bit n 为 1 表示 index 为 n 的观众有权限
    uint256 public privyBitmap;

    // 最大 256 人，索引 0-255
    AudienceInfo[256] public audienceList;
    mapping(address => uint256) public audienceIndex;
    // 记录地址是否已注册，区分 index 0 和未注册
    mapping(address => bool) public isRegistered;
    uint256 public nextAudienceIndex = 0;

    // --- 事件 ---
    event Joined(address indexed user, uint256 index);
    event VssKeyUpdated(address indexed user, Types.Hash newVssKeyCommitment);
    event DataKeyShared(uint256 newPrivyBitmap, uint256 newVersion);
    event DataKeyCommitmentUpdated(Types.Hash newCommitment);
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    constructor(Types.Pubkey memory _ownerPubKey) {
        owner = msg.sender;
        ownerPublicKey = _ownerPubKey;
    }

    // --- 内部逻辑 ---

    function _addAudience(
        address user,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) internal {
        require(nextAudienceIndex < 256, "Audience list full");
        require(!isRegistered[user], "Audience exists");

        uint256 idx = nextAudienceIndex++;
        audienceIndex[user] = idx;
        isRegistered[user] = true;

        audienceList[idx] = AudienceInfo({
            vssKeyCommitment: vssKeyCommitment,
            encryptedVssKey: encryptedVssKey
        });

        emit Joined(user, idx);
    }

    // --- 外部接口 ---

    function join(
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) external {
        _addAudience(msg.sender, vssKeyCommitment, encryptedVssKey);
    }

    function updateVssKey(
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) public {
        _updateVssKeyInternal(msg.sender, vssKeyCommitment, encryptedVssKey);
    }

    function _updateVssKeyInternal(
        address user,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) internal {
        require(isRegistered[user], "Not an audience");
        uint256 idx = audienceIndex[user];

        audienceList[idx].vssKeyCommitment = vssKeyCommitment;
        audienceList[idx].encryptedVssKey = encryptedVssKey;

        // Clear bit in bitmap to revoke access to old version
        privyBitmap &= ~(uint256(1) << idx);

        emit VssKeyUpdated(user, vssKeyCommitment);
    }

    function isPrivy(address user) public view returns (bool) {
        if (version == 0 || !isRegistered[user]) return false;

        uint256 idx = audienceIndex[user];
        return (privyBitmap & (uint256(1) << idx)) != 0;
    }

    function submitDataKeyCommitment(Types.Hash _commitment) public onlyOwner {
        dataKeyCommitment = _commitment;
        emit DataKeyCommitmentUpdated(_commitment);
    }

    /**
     * @notice 分发数据密钥并同步更新位图
     * @param audiences 此次获得授权的观众地址列表
     * @param encryptedDataKeys 对应的加密数据密钥
     */
    function shareDataKey(
        bytes calldata proof,
        bytes calldata publicValues,
        address[] memory audiences,
        Types.Cipher32[] memory encryptedDataKeys
    ) public onlyOwner {
        require(
            audiences.length == encryptedDataKeys.length,
            "Mismatched input"
        );

        // TODO verify public values
        // 1. origin msg hash
        // 2. vssKeyCommitment
        // 3. encryptedDataKeys
        require(
            verifier.verifyVSS(proof, publicValues),
            "VSS verification failed"
        );

        version += 1;

        uint256 newBitmap = 0;

        for (uint256 i = 0; i < audiences.length; i++) {
            address user = audiences[i];

            if (isRegistered[user]) {
                uint256 idx = audienceIndex[user];
                newBitmap |= (uint256(1) << idx);
            }
        }

        privyBitmap = newBitmap;

        emit DataKeyShared(newBitmap, version);
    }

    function transferOwner(address newOwner) public onlyOwner {
        require(newOwner != address(0), "Invalid address");
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }
}
