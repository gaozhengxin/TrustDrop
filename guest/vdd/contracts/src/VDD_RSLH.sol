// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

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

/// @title VDD_RSLH.
contract VDD_RSLH {
    using VDD_RSLH_PublicValues for bytes;

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
    ) public view returns (VDD_RSLH_PublicValues.VDD_RSLH_PublicValuesStruct memory) {
        ISP1Verifier(verifier).verifyProof(
            VDDProgramVKey,
            _publicValues,
            _proofBytes
        );
        VDD_RSLH_PublicValues.VDD_RSLH_PublicValuesStruct
            memory publicValues = _publicValues.decodeRSLHVE();
        return publicValues;
    }
}
