// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

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
        uint256 cipherLen = totalLen - 68; // 动态计算 CID 的长度

        assembly {
            let ptr := add(data, 32)

            mstore(r, mload(ptr))

            mstore(add(r, 32), mload(add(ptr, 32)))

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

/// @title VDD.
contract VDD {
    using VDDPublicValues for bytes;

    /// @notice The address of the SP1 verifier contract.
    /// @dev This can either be a specific SP1Verifier for a specific version, or the
    ///      SP1VerifierGateway which can be used to verify proofs for any version of SP1.
    ///      For the list of supported verifiers on each chain, see:
    ///      https://github.com/succinctlabs/sp1-contracts/tree/main/contracts/deployments
    address public verifier;

    /// @notice The verification key for the fibonacci program.
    bytes32 public VDDProgramVKey;

    constructor(address _verifier, bytes32 _VDDProgramVKey) {
        verifier = _verifier;
        VDDProgramVKey = _VDDProgramVKey;
    }

    /// @notice The entrypoint for verifying the proof of a fibonacci number.
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
        VDDPublicValues.VDDPublicValuesStruct
            memory publicValues = _publicValues.decodeVDD();
        return publicValues;
    }
}
