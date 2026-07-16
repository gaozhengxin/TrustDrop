// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

// lib/sp1-contracts/contracts/src/ISP1Verifier.sol

/// @title SP1 Verifier Interface
/// @author Succinct Labs
/// @notice This contract is the interface for the SP1 Verifier.
interface ISP1Verifier {
    /// @notice Verifies a proof with given public values and vkey.
    /// @dev It is expected that the first 4 bytes of proofBytes must match the first 4 bytes of
    /// target verifier's VERIFIER_HASH.
    /// @param programVKey The verification key for the RISC-V program.
    /// @param publicValues The public values encoded as bytes.
    /// @param proofBytes The proof of the program execution the SP1 zkVM encoded as bytes.
    function verifyProof(
        bytes32 programVKey,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external view;
}

interface ISP1VerifierWithHash is ISP1Verifier {
    /// @notice Returns the hash of the verifier.
    function VERIFIER_HASH() external pure returns (bytes32);
}

// src/VDD_RSLH.sol

// 保持接口一致性
interface IVDDVerifier {
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool);
}

library VDD_RSLH_PublicValues {
    struct VDD_RSLH_PublicValuesStruct {
        bytes32 cOrigin;
        bytes32 cKey;
        bytes cCipher;
    }

    function decodeRSLHVE(
        bytes memory data
    ) internal pure returns (VDD_RSLH_PublicValuesStruct memory r) {
        require(data.length >= 64, "Invalid RSLHVE data length");

        uint256 cipherLen = data.length - 64;

        assembly {
            let ptr := add(data, 32)
            // 存储 cOrigin
            mstore(r, mload(ptr))
            // 存储 cKey
            mstore(add(r, 32), mload(add(ptr, 32)))
        }

        // 提取剩余的动态 cCipher
        bytes memory cipherBytes = new bytes(cipherLen);
        for (uint256 i = 0; i < cipherLen; i++) {
            cipherBytes[i] = data[i + 64];
        }
        r.cCipher = cipherBytes;

        return r;
    }
}

/// @title VDD_RSLH
contract VDD_RSLH is IVDDVerifier {
    using VDD_RSLH_PublicValues for bytes;

    address public verifier;
    bytes32 public VDDProgramVKey;

    constructor(address _verifier, bytes32 _VDDProgramVKey) {
        verifier = _verifier;
        VDDProgramVKey = _VDDProgramVKey;
    }

    /**
     * @notice 实现 IVDDVerifier 接口。
     */
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external override returns (bool) {
        // 1. ZK 证明校验 (保持与原测试逻辑一致)
        this.verifyVDDProof(publicValues, proof);

        // 2. 解码并进行 Binding 检查
        VDD_RSLH_PublicValues.VDD_RSLH_PublicValuesStruct
            memory pv = publicValues.decodeRSLHVE();

        bytes32 computedHash = computeBindingHash(
            abi.encodePacked(pv.cOrigin),
            pv.cKey,
            pv.cCipher
        );

        require(computedHash == bindingHash, "VDD_RSLH: Binding hash mismatch");

        return true;
    }

    /// @notice The entrypoint for verifying the vdd proof.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyVDDProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    )
        public
        view
        returns (VDD_RSLH_PublicValues.VDD_RSLH_PublicValuesStruct memory)
    {
        ISP1Verifier(verifier).verifyProof(
            VDDProgramVKey,
            _publicValues,
            _proofBytes
        );
        return _publicValues.decodeRSLHVE();
    }

    /**
     * @notice Pure 函数，供测试使用，计算符合主流程要求的 bindingHash。
     */
    function computeBindingHash(
        bytes memory cOrigin,
        bytes32 dataKeyCommitment,
        bytes memory cCipher
    ) public pure returns (bytes32) {
        return keccak256(abi.encode(cOrigin, dataKeyCommitment, cCipher));
    }
}

