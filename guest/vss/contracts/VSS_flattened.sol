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

// src/VSS.sol

interface IVSSVerifier {
    function verifyVSS(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool);
}

library VSSPublicValues {
    struct VSSPublicValuesStruct {
        uint64 length;
        bytes32 hOrigBlock;
        bytes32[] cipherBlock;
        bytes32[] hKCommitment;
        bytes12[] nonce;
    }

    function decodeVSS(
        bytes memory data
    ) internal pure returns (VSSPublicValuesStruct memory r) {
        uint256 slot = 0;
        uint256 ptr;

        assembly {
            ptr := add(data, 32)
        }

        // ---- length (uint64) ----
        {
            uint256 w;
            assembly {
                w := mload(ptr)
            }
            r.length = uint64(w);
            slot += 1;
        }

        // ---- hOrigBlock ----
        {
            bytes32 w;
            assembly {
                w := mload(add(ptr, mul(slot, 32)))
            }
            r.hOrigBlock = w;
            slot += 1;
        }

        // ---- cipherBlock.length ----
        uint256 cipherLen;
        {
            uint256 w;
            assembly {
                w := mload(add(ptr, mul(slot, 32)))
            }
            cipherLen = w;
            slot += 1;
        }
        r.cipherBlock = new bytes32[](cipherLen);
        for (uint256 i = 0; i < cipherLen; i++) {
            bytes32 w;
            assembly {
                w := mload(add(ptr, mul(add(slot, i), 32)))
            }
            r.cipherBlock[i] = w;
        }
        slot += cipherLen;

        // ---- hKCommitment.length ----
        uint256 hkLen;
        {
            uint256 w;
            assembly {
                w := mload(add(ptr, mul(slot, 32)))
            }
            hkLen = w;
            slot += 1;
        }
        r.hKCommitment = new bytes32[](hkLen);
        for (uint256 i = 0; i < hkLen; i++) {
            bytes32 w;
            assembly {
                w := mload(add(ptr, mul(add(slot, i), 32)))
            }
            r.hKCommitment[i] = w;
        }
        slot += hkLen;

        // ---- nonce.length ----
        uint256 nonceLen;
        {
            uint256 w;
            assembly {
                w := mload(add(ptr, mul(slot, 32)))
            }
            nonceLen = w;
            slot += 1;
        }
        r.nonce = new bytes12[](nonceLen);
        for (uint256 i = 0; i < nonceLen; i++) {
            bytes32 w;
            assembly {
                w := mload(add(ptr, mul(add(slot, i), 32)))
            }
            r.nonce[i] = bytes12(w);
        }

        return r;
    }
}

contract VSS is IVSSVerifier {
    using VSSPublicValues for bytes;

    address public verifier;
    bytes32 public VSSProgramVKey;

    constructor(address _verifier, bytes32 _VSSProgramVKey) {
        verifier = _verifier;
        VSSProgramVKey = _VSSProgramVKey;
    }

    /**
     * @notice 实现 IVSSVerifier 接口，用于主流程调用。
     * @dev 增加了 bindingHash 的校验。
     */
    function verifyVSS(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external override returns (bool) {
        // 1. 底层 ZK 证明校验 (逻辑复用原有的 verifyVSSProof)
        this.verifyVSSProof(publicValues, proof);

        // 2. 解码并校验 BindingHash 一致性
        VSSPublicValues.VSSPublicValuesStruct memory pv = publicValues
            .decodeVSS();

        bytes32 computedHash = computeBindingHash(
            pv.hOrigBlock,
            pv.hKCommitment,
            pv.cipherBlock
        );

        require(computedHash == bindingHash, "VSS: Binding hash mismatch");

        return true;
    }

    /// @notice The entrypoint for verifying the proof of a fibonacci number.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyVSSProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public view returns (VSSPublicValues.VSSPublicValuesStruct memory) {
        ISP1Verifier(verifier).verifyProof(
            VSSProgramVKey,
            _publicValues,
            _proofBytes
        );
        return _publicValues.decodeVSS();
    }

    /**
     * @notice Pure 函数，用于在测试或外部计算 bindingHash。
     */
    function computeBindingHash(
        bytes32 hOrigBlock,
        bytes32[] memory hKCommitment,
        bytes32[] memory cipherBlock
    ) public pure returns (bytes32) {
        return keccak256(abi.encode(hOrigBlock, hKCommitment, cipherBlock));
    }
}

