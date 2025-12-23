// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

library VDDPublicValues {
    struct VDDPublicValuesStruct {
        bytes32 cOrigin;
        bytes32 cKey;
        bytes32 cCipher;
        uint256 dataLength;
    }

    function decodeVDD(
        bytes memory data
    ) internal pure returns (VDDPublicValuesStruct memory r) {
        require(data.length >= 100, "Invalid data length");

        uint256 ptr;
        assembly {
            ptr := add(data, 32)
        }

        // 1. cOrigin (32 bytes) - Offset 0
        assembly {
            r.cOrigin := mload(ptr)
        }

        // 2. cKey (32 bytes) - Offset 32
        assembly {
            r.cKey := mload(add(ptr, 32))
        }

        // 3. cCipher (32 bytes) - Offset 64
        assembly {
            r.cCipher := mload(add(ptr, 64))
        }

        // 4. dataLength (u32 -> 4 bytes) - Offset 96
        {
            uint256 w;
            assembly {
                w := mload(add(ptr, 96))
                w := shr(224, w)
            }
            r.dataLength = uint32(w);
        }

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
