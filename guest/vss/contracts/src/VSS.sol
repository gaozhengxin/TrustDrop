// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

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

        // ---- cipherBlock[i] ----
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

        // ---- hKCommitment[i] ----
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

        // ---- nonce[i] ----
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

/// @title VSS.
contract VSS {
    using VSSPublicValues for bytes;

    /// @notice The address of the SP1 verifier contract.
    /// @dev This can either be a specific SP1Verifier for a specific version, or the
    ///      SP1VerifierGateway which can be used to verify proofs for any version of SP1.
    ///      For the list of supported verifiers on each chain, see:
    ///      https://github.com/succinctlabs/sp1-contracts/tree/main/contracts/deployments
    address public verifier;

    /// @notice The verification key for the fibonacci program.
    bytes32 public hVSSProgramVKey;

    constructor(address _verifier, bytes32 _hVSSProgramVKey) {
        verifier = _verifier;
        hVSSProgramVKey = _hVSSProgramVKey;
    }

    /// @notice The entrypoint for verifying the proof of a fibonacci number.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyVSSProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public view returns (VSSPublicValues.VSSPublicValuesStruct memory) {
        ISP1Verifier(verifier).verifyProof(
            hVSSProgramVKey,
            _publicValues,
            _proofBytes
        );
        VSSPublicValues.VSSPublicValuesStruct
            memory publicValues = _publicValues.decodeVSS();
        return publicValues;
    }
}
