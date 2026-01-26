// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./lib/Types.sol";
import {Ownable} from "./lib/Ownable.sol";
import {Pausable} from "./lib/Pausable.sol";
import {IVSSVerifier} from "./interfaces/IVerifier.sol";

contract VSS is Pausable {
    using Types for *;

    struct AudienceInfo {
        Types.Hash vssKeyCommitment;
        Types.Cipher32 encryptedVssKey;
    }

    // --- 常量 ---
    uint256 public constant BUCKET_SIZE = 256;

    // --- 状态变量 ---
    IVSSVerifier public vssVerifier;
    Types.Pubkey public ownerPublicKey;
    Types.Hash public dataKeyCommitment;

    // 核心重构：从单 uint256 扩展为映射：bucketId => bitmap
    mapping(uint256 => uint256) public privyBitmaps;

    // 索引管理
    AudienceInfo[] public audienceList;
    mapping(address => uint256) public audienceIndex;
    mapping(address => bool) public isRegistered;

    // --- 事件 ---
    event Joined(address indexed user, uint256 index);
    event DataKeyShared(address[] audiences, Types.Cipher32[] encryptedDataKeys);
    event DataKeyCommitmentUpdated(Types.Hash newCommitment);

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _owner,
        address _vssVerifier
    ) Ownable(_owner) {
        ownerPublicKey = _ownerPubKey;
        vssVerifier = IVSSVerifier(_vssVerifier);
    }

    function init_VSS(
        Types.Pubkey memory _ownerPubKey,
        address _owner,
        address _vssVerifier
    ) internal {
        init_owner(_owner);
        ownerPublicKey = _ownerPubKey;
        vssVerifier = IVSSVerifier(_vssVerifier);
    }

    // --- 内部逻辑 ---

    function _addAudience(
        address user,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) internal {
        require(!isRegistered[user], "Audience exists");

        uint256 idx = audienceList.length;
        audienceIndex[user] = idx;
        isRegistered[user] = true;

        audienceList.push(
            AudienceInfo({
                vssKeyCommitment: vssKeyCommitment,
                encryptedVssKey: encryptedVssKey
            })
        );

        emit Joined(user, idx);
    }

    // --- 外部接口 ---

    function join(
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) external virtual whenNotPaused {
        _addAudience(msg.sender, vssKeyCommitment, encryptedVssKey);
    }

    function isPrivy(address user) public view returns (bool) {
        if (!isRegistered[user]) return false;

        uint256 idx = audienceIndex[user];
        uint256 bucketId = idx / BUCKET_SIZE;
        uint256 offset = idx % BUCKET_SIZE;

        return (privyBitmaps[bucketId] & (uint256(1) << offset)) != 0;
    }

    function submitDataKeyCommitment(Types.Hash _commitment) public onlyOwner {
        if (Types.Hash.unwrap(dataKeyCommitment) != bytes32(0)) {
            revert("Cannot submit data key commitment again");
        }
        dataKeyCommitment = _commitment;
        emit DataKeyCommitmentUpdated(_commitment);
    }

    /**
     * @notice 分发数据密钥并同步更新位图
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

        bytes32[] memory pubkeys = new bytes32[](audiences.length);
        for (uint256 i = 0; i < audiences.length; i++) {
            require(isRegistered[audiences[i]], "Unregistered");
            pubkeys[i] = audienceList[audienceIndex[audiences[i]]].vssKeyCommitment.unwrap();
        }

        bytes32 bindingHash = keccak256(
            abi.encode(dataKeyCommitment, pubkeys, encryptedDataKeys)
        );

        require(
            vssVerifier.verifyVSS(proof, publicValues, bindingHash),
            "VSS verification failed"
        );

        for (uint256 i = 0; i < audiences.length; i++) {
            address user = audiences[i];
            if (isRegistered[user]) {
                uint256 idx = audienceIndex[user];
                uint256 bucketId = idx / BUCKET_SIZE;
                uint256 offset = idx % BUCKET_SIZE;

                privyBitmaps[bucketId] |= (uint256(1) << offset);
            }
        }

        emit DataKeyShared(audiences, encryptedDataKeys);
    }
}
