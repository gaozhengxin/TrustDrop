// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./VSS.sol";
import "./interfaces/IOracleClient.sol";
import "./interfaces/IOracleProxy.sol";

contract VDD is VSS, IOracleClient {
    using Types for *;

    IOracleProxy public oracleWrapper;

    struct DataInfo {
        Types.DataCommitment commitment;
        uint256 size;
        string info;
        uint256 timestamp;
    }

    // 使用 commitment 的哈希值作为 key
    mapping(bytes32 => DataInfo) public dataInfoList;

    // State 1: vddVerified[cCipher] = true means ZK proof passed
    mapping(bytes => bool) public vddVerified;

    // State 2: oracleSuccessUntil[cCipher] = timestamp.
    mapping(bytes => uint256) public oracleSuccessUntil;

    event DataListed(bytes32 indexed dataId, uint256 size);
    event VDDProofSubmitted(bytes cCipher);

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper
    ) VSS(_ownerPubKey) {
        oracleWrapper = IOracleProxy(_oracleWrapper);
    }

    // 根据 commitment 原始字节查询
    function retrieveDataInfo(
        bytes memory commitment
    ) public view returns (DataInfo memory) {
        bytes32 dataId = keccak256(commitment);
        return dataInfoList[dataId];
    }

    // 由 Owner 上架数据元信息
    function listDataInfo(
        Types.DataCommitment memory _commitment,
        uint256 _size,
        string memory _info
    ) public onlyOwner {
        bytes32 dataId = keccak256(_commitment.data);
        dataInfoList[dataId] = DataInfo({
            commitment: _commitment,
            size: _size,
            info: _info,
            timestamp: block.timestamp
        });
        emit DataListed(dataId, _size);
    }

    // 提交 VDD 证明并触发 Oracle 检查
    function submitVDDProof(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes calldata dataCommitment,
        bytes memory cCipher // 加密后的密文，用于 Oracle 校验存储节点
    ) public onlyOwner {
        // 1. ZK Verification
        // TODO verify public values
        // 1. data commitment
        // 2. dataKeyCommitment
        // 3. cCipher
        require(verifier.verifyVDD(proof, publicValues), "VDD verification failed");

        vddVerified[cCipher] = true;

        oracleWrapper.request(cCipher);
        emit VDDProofSubmitted(cCipher);
    }

    function triggerOracle(bytes memory cCipher) public onlyOwner {
        require(vddVerified[cCipher], "VDD not verified");
        oracleWrapper.request(cCipher);
    }

    // Oracle 异步回调：验证成功
    function onSuccess(bytes calldata cCipher) external virtual {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        // TODO let oracle pass in actual life span
        oracleSuccessUntil[cCipher] = block.timestamp + 30 days;
    }

    // Oracle 异步回调：验证失败
    function onFail(bytes calldata cCipher) external virtual {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = 0;
    }
}
