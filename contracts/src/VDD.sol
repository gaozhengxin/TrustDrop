// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./VSS.sol";
import {Types} from "./lib/Types.sol";
import "./interfaces/IOracleClient.sol";
import "./interfaces/IOracleProxy.sol";
import {IVDDVerifier} from "./interfaces/IVerifier.sol";

contract VDD is VSS, IOracleClient {
    using Types for *;

    IOracleProxy public oracleWrapper;

    struct DataInfo {
        Types.DataCommitment commitment;
        uint256 timestamp;
    }

    // --- zk 验证器 ---
    IVDDVerifier public vddVerifier;

    // 使用 commitment 的哈希值作为 key
    mapping(bytes32 => DataInfo) public dataInfoList;

    // State 1: vddVerified[cCipher] = true means ZK proof passed
    mapping(bytes => bool) public vddVerified;

    // State 2: oracleSuccessUntil[cCipher] = timestamp.
    mapping(bytes => uint256) public oracleSuccessUntil;

    uint256 public immutable GRACE_PERIOD = 1 days;

    // lastOracleRequestAt[cCipher] = timestamp
    mapping(bytes => uint256) public lastOracleRequestAt;

    mapping(bytes32 => uint256) public dataReferenceCount;

    uint256 public constant ORACLE_COOLDOWN = 1 minutes;

    event DataListed(bytes32 indexed dataId);
    event DataDelisted(bytes32 indexed dataId);
    event VDDProofSubmitted(bytes cCipher);
    event OracleRequestSkipped(bytes cCipher, string msg);

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _owner,
        address _vssVerifier,
        address _vddVerifier
    ) VSS(_ownerPubKey, _owner, _vssVerifier) {
        oracleWrapper = IOracleProxy(_oracleWrapper);
        vddVerifier = IVDDVerifier(_vddVerifier);
    }

    function init_VDD(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _owner,
        address _vssVerifier,
        address _vddVerifier
    ) internal {
        init_VSS(_ownerPubKey, _owner, _vssVerifier);
        oracleWrapper = IOracleProxy(_oracleWrapper);
        vddVerifier = IVDDVerifier(_vddVerifier);
    }

    function getDataId(
        bytes memory dataCommitment
    ) public pure returns (bytes32) {
        return keccak256(dataCommitment);
    }

    // 根据 commitment 原始字节查询
    function retrieveDataInfoById(
        bytes memory commitment
    ) public view returns (DataInfo memory) {
        bytes32 dataId = getDataId(commitment);
        return dataInfoList[dataId];
    }

    // 由 Owner 上架数据元信息
    function listDataInfo(
        Types.DataCommitment memory _commitment
    ) public onlyOwner returns (bytes32) {
        return _listDataInfo(_commitment);
    }

    function _listDataInfo(
        Types.DataCommitment memory _commitment
    ) internal whenNotPaused returns (bytes32) {
        bytes32 dataId = getDataId(_commitment.data);
        if (dataReferenceCount[dataId] == 0) {
            dataInfoList[dataId] = DataInfo({
                commitment: _commitment,
                timestamp: block.timestamp
            });
        }
        dataReferenceCount[dataId]++;
        emit DataListed(dataId);
        return dataId;
    }

    /**
     * @notice Owner cannot delist data directly, extra logic required.
     */
    function _delistDataInfo(bytes32 dataId) internal whenNotPaused {
        dataReferenceCount[dataId]--;
        if (dataReferenceCount[dataId] == 0) {
            delete dataInfoList[dataId];
        }
        emit DataDelisted(dataId);
    }

    // 提交 VDD 证明并触发 Oracle 检查
    function submitVDDProof(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes calldata cOrigin,
        bytes memory cCipher // 加密后的密文，用于 Oracle 校验存储节点
    ) public onlyOwner {
        bytes32 bindHash = keccak256(
            abi.encode(cOrigin, dataKeyCommitment, cCipher)
        );
        // ======
        // 1. ZK Verification
        require(
            vddVerifier.verifyVDD(proof, publicValues, bindHash),
            "VDD verification failed"
        );

        vddVerified[cCipher] = true;

        _triggerOracle(cCipher);
        emit VDDProofSubmitted(cCipher);
    }

    function triggerOracle(bytes memory cCipher) public onlyOwner {
        _triggerOracle(cCipher);
    }

    function _triggerOracle(bytes memory cCipher) internal {
        require(vddVerified[cCipher], "VDD not verified");

        if (block.timestamp < lastOracleRequestAt[cCipher] + ORACLE_COOLDOWN) {
            emit OracleRequestSkipped(cCipher, "Cooldown active");
            return;
        }

        lastOracleRequestAt[cCipher] = block.timestamp;
        oracleWrapper.request(cCipher, address(this));
    }

    function onResponse(
        bytes memory cCipher,
        bytes memory response
    ) external virtual {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");

        // 1. 基础长度校验，防止 abi.decode 溢出或报错
        require(response.length == 64, "Invalid response length");

        (uint256 status, uint256 endTime) = abi.decode(
            response,
            (uint256, uint256)
        );

        // 2. 业务边界校验（注入防范）
        // 防止 Oracle 返回一个极大的时间戳导致系统逻辑溢出
        require(endTime < block.timestamp + 10 * 365 days, "EndTime too far");

        // 3. 状态校验
        if (status > 2) revert("Unknown status from oracle");

        delete lastOracleRequestAt[cCipher];

        if (status == 2) {
            // Ensured
            onSuccess(cCipher, endTime);
        }
        if (status == 1) {
            // Retriveable
            onSuccess(cCipher, block.timestamp + GRACE_PERIOD);
        }
        if (status == 0) {
            // Not retrievable
            onFail(cCipher);
        }
    }

    // Oracle 异步回调：验证成功
    function onSuccess(bytes memory cCipher, uint256 endTime) internal {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = endTime;
    }

    // Oracle 异步回调：验证失败
    function onFail(bytes memory cCipher) internal {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = 0;
    }
}
