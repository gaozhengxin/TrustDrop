// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

library HVSSPublicValues {
    struct HVSSPublicValuesStruct {
        uint64 length;
        bytes32 hOrigBlock;
        bytes32[] cipherBlock;
        bytes[] hKCommitment;
        uint32[] nonce;
    }

    // 提供一个 decode 工具函数，哪个合约想用直接调用
    function decode(
        bytes memory publicValues
    ) internal pure returns (HVSSPublicValuesStruct memory) {
        return abi.decode(publicValues, (HVSSPublicValuesStruct));
    }
}

/// @title HVSS.
contract HVSS {
    using HVSSPublicValues for bytes;

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
    function verifyHVSSProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public view returns (HVSSPublicValues.HVSSPublicValuesStruct memory) {
        ISP1Verifier(verifier).verifyProof(
            hVSSProgramVKey,
            _publicValues,
            _proofBytes
        );
        HVSSPublicValues.HVSSPublicValuesStruct memory publicValues = _publicValues.decode();
        return publicValues;
    }
}
