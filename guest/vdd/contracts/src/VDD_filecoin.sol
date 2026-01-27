// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

interface IVDDVerifier {
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool);
}

library VDDPublicValues {
    struct VDDPublicValuesStruct {
        bytes32 cOrigin;
        bytes32 cKey;
        uint256 dataLength;
        bytes cCipher;
    }

    function decodeVDD(
        bytes memory data
    ) internal pure returns (VDDPublicValuesStruct memory r) {
        require(data.length >= 68, "Invalid data length");

        uint256 totalLen = data.length;
        uint256 cipherLen = totalLen - 68;

        assembly {
            let ptr := add(data, 32)
            // cOrigin (bytes32)
            mstore(r, mload(ptr))
            // cKey (bytes32)
            mstore(add(r, 32), mload(add(ptr, 32)))
            // dataLength (uint256) - 从末尾取 4 字节转 uint32/256
            let lenPtr := add(ptr, sub(totalLen, 4))
            let w := mload(lenPtr)
            w := shr(224, w)
            mstore(add(r, 64), w)
        }

        bytes memory cipherBytes = new bytes(cipherLen);
        for (uint256 i = 0; i < cipherLen; i++) {
            cipherBytes[i] = data[i + 64];
        }
        r.cCipher = cipherBytes;

        return r;
    }
}

contract VDD is IVDDVerifier {
    using VDDPublicValues for bytes;

    address public verifier;
    bytes32 public VDDProgramVKey;

    constructor(address _verifier, bytes32 _VDDProgramVKey) {
        verifier = _verifier;
        VDDProgramVKey = _VDDProgramVKey;
    }

    /**
     * @notice 实现 IVDDVerifier 接口，用于主流程调用。
     */
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external override returns (bool) {
        // 1. 底层 ZK 证明校验
        this.verifyVDDProof(publicValues, proof);

        // 2. 解码并校验 BindingHash 一致性
        VDDPublicValues.VDDPublicValuesStruct memory pv = publicValues
            .decodeVDD();

        bytes32 computedHash = computeBindingHash(
            abi.encodePacked(pv.cOrigin),
            pv.cKey,
            pv.cCipher
        );

        require(computedHash == bindingHash, "VDD: Binding hash mismatch");

        return true;
    }

    /// @notice The entrypoint for verifying the vdd proof.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyVDDProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public view returns (VDDPublicValues.VDDPublicValuesStruct memory) {
        ISP1Verifier(verifier).verifyProof(
            VDDProgramVKey,
            _publicValues,
            _proofBytes
        );
        return _publicValues.decodeVDD();
    }

    /**
     * @notice Pure 函数，用于在测试或外部计算 bindingHash。
     * @param cOrigin 原始数据标识 (bytes)
     * @param dataKeyCommitment 密钥承诺 (bytes32)
     * @param cCipher 密文 (bytes)
     */
    function computeBindingHash(
        bytes memory cOrigin,
        bytes32 dataKeyCommitment,
        bytes memory cCipher
    ) public pure returns (bytes32) {
        return keccak256(abi.encode(cOrigin, dataKeyCommitment, cCipher));
    }
}
