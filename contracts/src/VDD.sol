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

    uint256 public immutable GRACE_PERIOD = 3 days;

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
        require(
            verifier.verifyVDD(proof, publicValues),
            "VDD verification failed"
        );

        vddVerified[cCipher] = true;

        oracleWrapper.request(cCipher, address(this));
        emit VDDProofSubmitted(cCipher);
    }

    function triggerOracle(bytes memory cCipher) public onlyOwner {
        _triggerOracle(cCipher);
    }

    function _triggerOracle(bytes memory cCipher) internal {
        require(vddVerified[cCipher], "VDD not verified");
        oracleWrapper.request(cCipher, address(this));
    }

    function onResponse(
        bytes memory cCipher,
        bytes memory response
    ) external virtual {
        if (response.length >= 64) {
            (uint256 status, uint256 endTime) = abi.decode(
                response,
                (uint256, uint256)
            );
            assert(endTime < block.timestamp + 1000 days);
            if (status == 2) {
                // Ensured
                onFail(cCipher);
            }
            if (status == 1) {
                // Retriveable
                onSuccess(cCipher, block.timestamp + GRACE_PERIOD);
            }
            if (status == 0) {
                // Not retrievable
                onSuccess(cCipher, endTime);
            }
        }
    }

    // Oracle 异步回调：验证成功
    function onSuccess(bytes memory cCipher, uint256 endTime) internal {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = block.timestamp + endTime;
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
